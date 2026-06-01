//! Port forwarding over Iroh QUIC (same stack as file transfer — hole punch + relay).
//!
//! TCP: each local TCP connection = one Iroh bidirectional stream.
//! UDP: unreliable datagrams on the connection (when enabled).

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use anyhow::Context;
use iroh::endpoint::Connection;
use iroh::{Endpoint, EndpointAddr, Watcher};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::Mutex as TokioMutex;

pub const MAX_STREAMS: u32              = 50;
pub const CONNECT_TIMEOUT: Duration    = Duration::from_secs(120);
pub const UDP_MAX_DATAGRAM: usize      = 65535;
pub const FORWARD_ALPN: &[u8]         = b"punch/forward/1";

/// Copy buffer for TCP tunnel (matches transfer tuning).
pub const IO_COPY_BUFFER: usize = 256 * 1024;

/// Connect to `port` on loopback. Tries IPv4 then IPv6 — tools like Vite often bind `[::1]` only.
async fn tcp_connect_loopback(port: u16) -> anyhow::Result<TcpStream> {
    let candidates = [
        SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        SocketAddr::from((Ipv6Addr::LOCALHOST, port)),
    ];
    let mut last = None;
    for addr in candidates {
        match TcpStream::connect(addr).await {
            Ok(tcp) => return Ok(tcp),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("no loopback candidate")))
        .with_context(|| {
            format!(
                "Could not connect to 127.0.0.1:{} or [::1]:{} — is a server listening? (Vite: try `vite --host 127.0.0.1` if this keeps failing)",
                port, port
            )
        })
}

// ─── METADATA ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardHandshake {
    pub allowed_port: u16,
    pub protocol: ForwardProtocol,
    pub max_streams: u32,
    /// User-verifiable fingerprint (short hash of endpoint identity).
    pub session_fingerprint: String,
    pub token_type: String,
    /// JSON-serialized [`EndpointAddr`] for the connector to dial.
    pub endpoint_addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ForwardProtocol {
    Tcp,
    Udp,
    Both,
}

impl std::fmt::Display for ForwardProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ForwardProtocol::Tcp  => write!(f, "TCP"),
            ForwardProtocol::Udp  => write!(f, "UDP"),
            ForwardProtocol::Both => write!(f, "TCP+UDP"),
        }
    }
}

/// Short verbal fingerprint from endpoint identity (stable for a session).
pub fn session_fingerprint(addr: &EndpointAddr) -> String {
    let hash = Sha256::digest(format!("{}", addr.id).as_bytes());
    let hex  = format!("{:x}", hash);
    format!("{}-{}-{}", &hex[0..4], &hex[4..8], &hex[8..12])
}

// ─── EXPOSER ─────────────────────────────────────────────────────────────────

pub async fn prepare_exposer(
    port: u16,
    protocol: ForwardProtocol,
    token_type: &str,
) -> anyhow::Result<(ForwardHandshake, Endpoint)> {
    print!("🔌 Starting Iroh endpoint... ");
    let endpoint = Endpoint::builder()
        .alpns(vec![FORWARD_ALPN.to_vec()])
        .bind()
        .await
        .context("Failed to create Iroh endpoint")?;

    endpoint.online().await;

    let addr: EndpointAddr = endpoint.watch_addr().get();
    println!("done");
    println!("🌐 Node ID: {}", addr.id);

    let fingerprint = session_fingerprint(&addr);
    println!("🔐 Session fingerprint: {}", fingerprint);
    println!("   Ask connector to verify this fingerprint before accepting.");

    let endpoint_addr = serde_json::to_string(&addr).context("serialize EndpointAddr")?;

    let handshake = ForwardHandshake {
        allowed_port: port,
        protocol,
        max_streams: MAX_STREAMS,
        session_fingerprint: fingerprint,
        token_type: token_type.to_string(),
        endpoint_addr,
    };

    Ok((handshake, endpoint))
}

pub async fn run_exposer(
    endpoint: Endpoint,
    handshake: &ForwardHandshake,
    active_streams: Arc<AtomicU32>,
) -> anyhow::Result<()> {
    println!("\n✅ Ready. Waiting for connector...\n");

    let conn = tokio::time::timeout(
        Duration::from_secs(120),
        async {
            loop {
                match endpoint.accept().await {
                    Some(incoming) => match incoming.await {
                        Ok(c) => return Ok(c),
                        Err(e) => tracing::warn!("Accept error: {}", e),
                    },
                    None => anyhow::bail!("Endpoint closed"),
                }
            }
        },
    )
    .await
    .context("Timed out waiting for connector")??;

    println!(
        "⚡ Connector connected — forwarding port {} ({})",
        handshake.allowed_port, handshake.protocol
    );
    println!("   Session: {}", handshake.session_fingerprint);
    println!("   Press Ctrl+C to stop.\n");

    verify_handshake_exposer(&conn, handshake).await?;

    let forward_id = uuid::Uuid::new_v4().to_string();
    crate::active_state::register_forward(crate::active_state::ActiveForward {
        id: forward_id.clone(),
        port: handshake.allowed_port,
        protocol: handshake.protocol.to_string(),
        token_type: handshake.token_type.clone(),
        fingerprint: handshake.session_fingerprint.clone(),
        started_at: chrono::Utc::now().to_rfc3339(),
        stream_count: 0,
    })
    .await;

    let port     = handshake.allowed_port;
    let protocol = handshake.protocol.clone();

    if protocol == ForwardProtocol::Tcp || protocol == ForwardProtocol::Both {
        let conn_tcp    = conn.clone();
        let streams_tcp = Arc::clone(&active_streams);
        let forward_id_tcp = forward_id.clone();
        tokio::spawn(async move {
            loop {
                match conn_tcp.accept_bi().await {
                    Ok((send, recv)) => {
                        let count = streams_tcp.fetch_add(1, Ordering::SeqCst) + 1;
                        crate::active_state::update_forward_streams(&forward_id_tcp, count).await;
                        println!("  → TCP stream opened ({} active)", count);
                        let streams_clone = Arc::clone(&streams_tcp);
                        let fid_spawn = forward_id_tcp.clone();
                        tokio::spawn(async move {
                            if let Err(e) = forward_tcp_stream(send, recv, port).await {
                                eprintln!(
                                    "  ⚠️  TCP tunnel ended: {} — check 127.0.0.1:{} and [::1]:{} (some dev servers listen on IPv6 only).",
                                    e, port, port
                                );
                                tracing::warn!(error = %e, "TCP forward stream");
                            }
                            let remaining = streams_clone.fetch_sub(1, Ordering::SeqCst) - 1;
                            crate::active_state::update_forward_streams(&fid_spawn, remaining).await;
                            println!("  ← TCP stream closed ({} active)", remaining);
                        });
                    }
                    Err(e) => {
                        tracing::debug!("TCP accept ended: {}", e);
                        break;
                    }
                }
            }
        });
    }

    if protocol == ForwardProtocol::Udp || protocol == ForwardProtocol::Both {
        let conn_udp = conn.clone();
        tokio::spawn(async move {
            if let Err(e) = forward_udp_exposer(conn_udp, port).await {
                tracing::debug!("UDP forwarder ended: {}", e);
            }
        });
    }

    tokio::signal::ctrl_c().await?;
    println!("\nStopping port forward.");
    crate::active_state::deregister_forward(&forward_id).await;
    endpoint.close().await;
    Ok(())
}

async fn forward_tcp_stream(
    mut quinn_send: iroh::endpoint::SendStream,
    mut quinn_recv: iroh::endpoint::RecvStream,
    port: u16,
) -> anyhow::Result<()> {
    let tcp = tcp_connect_loopback(port).await?;

    let (mut tcp_read, mut tcp_write) = tcp.into_split();

    let quinn_to_tcp = tokio::spawn(async move {
        let mut buf = vec![0u8; IO_COPY_BUFFER];
        loop {
            let n = match quinn_recv.read(&mut buf).await {
                Ok(Some(0)) | Ok(None) => break,
                Ok(Some(n)) => n,
                Err(e) => {
                    tracing::warn!(error = %e, "quinn→tcp read (exposer)");
                    break;
                }
            };
            tcp_write.write_all(&buf[..n]).await?;
        }
        let _ = tcp_write.shutdown().await;
        Ok::<_, anyhow::Error>(())
    });

    let tcp_to_quinn = tokio::spawn(async move {
        let mut buf = vec![0u8; IO_COPY_BUFFER];
        loop {
            let n = tcp_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            quinn_send.write_all(&buf[..n]).await?;
        }
        let _ = quinn_send.finish();
        Ok::<_, anyhow::Error>(())
    });

    let (r1, r2) = tokio::join!(quinn_to_tcp, tcp_to_quinn);
    r1.context("quinn→tcp task")??;
    r2.context("tcp→quinn task")??;
    Ok(())
}

async fn forward_udp_exposer(conn: Connection, port: u16) -> anyhow::Result<()> {
    let local_udp = UdpSocket::bind("127.0.0.1:0").await?;
    local_udp.connect(format!("127.0.0.1:{}", port)).await?;
    let local_udp = Arc::new(local_udp);

    println!("  📡 UDP forwarding active on local port {}", port);

    let udp_recv = Arc::clone(&local_udp);
    let conn_send = conn.clone();

    let local_to_quinn = async move {
        let mut buf = vec![0u8; UDP_MAX_DATAGRAM];
        loop {
            let n = udp_recv.recv(&mut buf).await?;
            conn_send.send_datagram(bytes::Bytes::copy_from_slice(&buf[..n]))?;
        }
        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    };

    let quinn_to_local = async move {
        loop {
            let data = conn.read_datagram().await?;
            local_udp.send(&data).await?;
        }
        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    };

    tokio::select! {
        r = local_to_quinn => r?,
        r = quinn_to_local => r?,
    }

    Ok(())
}

// ─── CONNECTOR ───────────────────────────────────────────────────────────────

pub async fn run_connector(handshake: &ForwardHandshake, local_port: u16) -> anyhow::Result<()> {
    println!("\n🔗 Connecting to exposer (Iroh)...");

    let endpoint_addr: EndpointAddr = serde_json::from_str(&handshake.endpoint_addr)
        .context("Invalid EndpointAddr in handshake")?;

    print!("🔌 Starting Iroh endpoint... ");
    let endpoint = Endpoint::builder()
        .alpns(vec![FORWARD_ALPN.to_vec()])
        .bind()
        .await
        .context("Failed to bind Iroh client")?;
    endpoint.online().await;
    println!("done");

    let conn = tokio::time::timeout(
        CONNECT_TIMEOUT,
        endpoint.connect(endpoint_addr, FORWARD_ALPN),
    )
    .await
    .context("Connection timed out")?
    .context("Failed to connect to exposer")?;

    println!("✅ Connected (Iroh QUIC)");

    verify_handshake_connector(&conn, handshake).await?;

    let proto = &handshake.protocol;
    let port  = handshake.allowed_port;

    println!("🔀 Forwarding:");
    if *proto == ForwardProtocol::Tcp || *proto == ForwardProtocol::Both {
        eprintln!("   TCP: http://127.0.0.1:{} → remote {}", local_port, port);
    }
    if *proto == ForwardProtocol::Udp || *proto == ForwardProtocol::Both {
        println!("   UDP: localhost:{} → remote:{}", local_port, port);
    }
    println!("   Fingerprint: {}", handshake.session_fingerprint);
    println!("   Press Ctrl+C to disconnect.\n");

    if *proto == ForwardProtocol::Tcp || *proto == ForwardProtocol::Both {
        let conn_tcp = conn.clone();
        let tcp_listener = TcpListener::bind(format!("127.0.0.1:{}", local_port))
            .await
            .with_context(|| format!("Could not bind local port {}", local_port))?;

        tokio::spawn(async move {
            loop {
                match tcp_listener.accept().await {
                    Ok((tcp, _)) => {
                        let conn_clone = conn_tcp.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_local_tcp(tcp, conn_clone).await {
                                tracing::warn!(error = %e, "local TCP handler");
                            }
                        });
                    }
                    Err(e) => {
                        tracing::warn!("TCP listener error: {}", e);
                        break;
                    }
                }
            }
        });
    }

    if *proto == ForwardProtocol::Udp || *proto == ForwardProtocol::Both {
        let conn_udp  = conn.clone();
        let udp_local = UdpSocket::bind(format!("127.0.0.1:{}", local_port))
            .await
            .with_context(|| format!("Could not bind local UDP port {}", local_port))?;

        tokio::spawn(async move {
            if let Err(e) = handle_local_udp(udp_local, conn_udp).await {
                tracing::debug!("UDP handler ended: {}", e);
            }
        });
    }

    tokio::signal::ctrl_c().await?;
    println!("\n🛑 Disconnecting.");
    endpoint.close().await;
    Ok(())
}

async fn handle_local_tcp(tcp: TcpStream, conn: Connection) -> anyhow::Result<()> {
    let (mut quinn_send, mut quinn_recv) = conn
        .open_bi()
        .await
        .context("Could not open Iroh stream — may be at stream limit")?;

    let (mut tcp_read, mut tcp_write) = tcp.into_split();

    let tcp_to_quinn = tokio::spawn(async move {
        let mut buf = vec![0u8; IO_COPY_BUFFER];
        loop {
            let n = tcp_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            quinn_send.write_all(&buf[..n]).await?;
        }
        let _ = quinn_send.finish();
        Ok::<_, anyhow::Error>(())
    });

    let quinn_to_tcp = tokio::spawn(async move {
        let mut buf = vec![0u8; IO_COPY_BUFFER];
        loop {
            let n = match quinn_recv.read(&mut buf).await {
                Ok(Some(0)) | Ok(None) => break,
                Ok(Some(n)) => n,
                Err(e) => {
                    tracing::warn!(error = %e, "quinn→tcp read (connector)");
                    break;
                }
            };
            tcp_write.write_all(&buf[..n]).await?;
        }
        let _ = tcp_write.shutdown().await;
        Ok::<_, anyhow::Error>(())
    });

    let (r1, r2) = tokio::join!(tcp_to_quinn, quinn_to_tcp);
    r1.context("tcp→quinn task")??;
    r2.context("quinn→tcp task")??;
    Ok(())
}

async fn handle_local_udp(socket: UdpSocket, conn: Connection) -> anyhow::Result<()> {
    let socket = Arc::new(socket);
    let last_peer: Arc<TokioMutex<Option<std::net::SocketAddr>>> = Arc::new(TokioMutex::new(None));

    let udp_recv = Arc::clone(&socket);
    let conn_send = conn.clone();
    let last_peer_tx = Arc::clone(&last_peer);

    let local_to_quinn = async move {
        let mut buf = vec![0u8; UDP_MAX_DATAGRAM];
        loop {
            let (n, peer) = udp_recv.recv_from(&mut buf).await?;
            *last_peer_tx.lock().await = Some(peer);
            conn_send.send_datagram(bytes::Bytes::copy_from_slice(&buf[..n]))?;
        }
        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    };

    let socket_rx = Arc::clone(&socket);
    let last_peer_rx = Arc::clone(&last_peer);

    let quinn_to_local = async move {
        loop {
            let data = conn.read_datagram().await?;
            let peer = *last_peer_rx.lock().await;
            if let Some(peer) = peer {
                socket_rx.send_to(&data, peer).await?;
            }
        }
        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    };

    tokio::select! {
        r = local_to_quinn => r?,
        r = quinn_to_local => r?,
    }

    Ok(())
}

// ─── HANDSHAKE (in-band over first bidi stream) ──────────────────────────────

async fn verify_handshake_exposer(conn: &Connection, handshake: &ForwardHandshake) -> anyhow::Result<()> {
    let (mut send, mut recv) = conn.open_bi().await?;

    let json = serde_json::to_vec(handshake)?;
    send.write_u32(json.len() as u32).await?;
    send.write_all(&json).await?;

    let ack_len = recv.read_u32().await.context("Handshake failed — no ACK")?;
    let mut ack_buf = vec![0u8; ack_len as usize];
    recv.read_exact(&mut ack_buf).await?;

    let acked_port: u16 = serde_json::from_slice(&ack_buf).context("Invalid handshake ACK")?;

    if acked_port != handshake.allowed_port {
        anyhow::bail!(
            "Port mismatch in handshake — expected {}, got {}",
            handshake.allowed_port, acked_port
        );
    }

    let _ = send.finish();
    tracing::debug!("Handshake verified — port {} confirmed", acked_port);
    Ok(())
}

async fn verify_handshake_connector(conn: &Connection, expected: &ForwardHandshake) -> anyhow::Result<()> {
    let (mut send, mut recv) = conn.accept_bi().await?;

    let len = recv.read_u32().await.context("No handshake received")?;
    let mut buf = vec![0u8; len as usize];
    recv.read_exact(&mut buf).await?;

    let received: ForwardHandshake = serde_json::from_slice(&buf).context("Invalid handshake")?;

    if received.allowed_port != expected.allowed_port {
        anyhow::bail!(
            "⚠️  Port mismatch — expected {}, server claims {}. Possible MITM.",
            expected.allowed_port, received.allowed_port
        );
    }

    if received.session_fingerprint != expected.session_fingerprint {
        anyhow::bail!("⚠️  Fingerprint mismatch — possible MITM attack. Aborting.");
    }

    if received.endpoint_addr != expected.endpoint_addr {
        tracing::warn!("EndpointAddr in handshake differed from signalling copy (ignored if connect succeeded)");
    }

    let ack = serde_json::to_vec(&received.allowed_port)?;
    send.write_u32(ack.len() as u32).await?;
    send.write_all(&ack).await?;
    let _ = send.finish();

    println!("✅ Handshake verified — port {} confirmed", received.allowed_port);
    Ok(())
}

// ─── AUDIT LOG ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ForwardAuditEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub role: String,
    pub port: u16,
    pub protocol: String,
    pub token_type: String,
    pub fingerprint: String,
    pub streams_opened: u32,
    pub bytes_forwarded: u64,
    pub ended_at: chrono::DateTime<chrono::Utc>,
}

pub async fn write_forward_log(entry: ForwardAuditEntry) -> anyhow::Result<()> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let path = home.join(".punch").join("logs").join("forward.json");

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut entries: Vec<ForwardAuditEntry> = if path.exists() {
        let content = tokio::fs::read_to_string(&path).await?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        vec![]
    };

    entries.push(entry);

    tokio::fs::write(&path, serde_json::to_string_pretty(&entries)?).await?;
    Ok(())
}
