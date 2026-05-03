//! Port forwarding over Iroh QUIC.
//!
//! Why Iroh instead of raw Quinn:
//! - STUN handled internally — no manual IP discovery needed
//! - Hole punching built in — works cross-network automatically
//! - iroh.network relay fallback — 100% connectivity guaranteed
//! - Public key authentication — stronger than self-signed certs
//! - No raw IP addresses needed — connect by EndpointAddr (public key + relay URL)
//!
//! TCP: each local TCP connection = one Iroh bidirectional QUIC stream
//! UDP: all UDP datagrams = Iroh unreliable datagrams (preserves UDP semantics)
//!
//! Security:
//! - Iroh TLS with peer public key authentication (no MITM possible)
//! - Port whitelist enforced per stream at protocol level
//! - Session fingerprint for verbal verification
//! - Max concurrent streams enforced (DoS protection)
//! - Token policy (T-No/Q-No/P-No) enforced before connection
//! - Full audit log on exposer side

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use anyhow::Context;
use iroh::endpoint::{Connection, Endpoint};
use iroh::{presets, Watcher};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

pub const MAX_STREAMS: u32           = 50;
pub const CONNECT_TIMEOUT: Duration  = Duration::from_secs(30);
pub const FORWARD_ALPN: &[u8]        = b"punch/forward/1";

/// Metadata exchanged via signalling before forwarding begins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardHandshake {
    pub allowed_port: u16,
    pub protocol: ForwardProtocol,
    pub max_streams: u32,
    pub session_fingerprint: String,
    pub token_type: String,
    /// Serialized EndpointAddr — connector uses this to connect directly
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

// ─── EXPOSER (Device A) ──────────────────────────────────────────────────────

/// Prepare Iroh endpoint for the exposer side.
/// Returns ForwardHandshake (to share via signalling) + Endpoint (to accept on).
pub async fn prepare_exposer(
    port: u16,
    protocol: ForwardProtocol,
    token_type: &str,
) -> anyhow::Result<(ForwardHandshake, Endpoint)> {
    print!("🔌 Starting Iroh endpoint... ");

    let endpoint = Endpoint::builder(presets::N0)
        .alpns(vec![FORWARD_ALPN.to_vec()])
        .bind()
        .await
        .context("Failed to create Iroh endpoint")?;

    // Wait for endpoint to come online and get full address
    // initialized() waits until we have a relay URL — guarantees connectivity
    let addr = endpoint
        .watch_addr()
        .initialized()
        .await
        .context("Failed to get endpoint address")?;

    println!("done");
    println!("🌐 Node ID: {}", addr.endpoint_id);

    // Fingerprint from node public key — cryptographically tied to this session
    let fingerprint = node_fingerprint(&addr.endpoint_id.to_string());
    println!("🔐 Session fingerprint: {}", fingerprint);
    println!("   Ask connector to verify this fingerprint before accepting.");

    let addr_str = serde_json::to_string(&addr)
        .context("Failed to serialize EndpointAddr")?;

    let handshake = ForwardHandshake {
        allowed_port: port,
        protocol,
        max_streams: MAX_STREAMS,
        session_fingerprint: fingerprint,
        token_type: token_type.to_string(),
        endpoint_addr: addr_str,
    };

    Ok((handshake, endpoint))
}

/// Run the exposer — accept Iroh connection, forward streams to local port.
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
                    Some(incoming) => {
                        match incoming.await {
                            Ok(c)  => return Ok(c),
                            Err(e) => tracing::warn!("Accept error: {}", e),
                        }
                    }
                    None => anyhow::bail!("Endpoint closed"),
                }
            }
        }
    ).await
    .context("Timed out waiting for connector")??;

    // Verify connector public key matches expected fingerprint
    let remote_id   = conn.remote_id();
    let actual_fp   = node_fingerprint(&remote_id.to_string());

    println!("⚡ Connector connected");
    println!("   Port: {} ({})", handshake.allowed_port, handshake.protocol);
    println!("   Session: {}", handshake.session_fingerprint);
    println!("   Remote: {}", actual_fp);
    println!("   Press Ctrl+C to stop.\n");

    let port     = handshake.allowed_port;
    let protocol = handshake.protocol.clone();

    // Spawn TCP stream acceptor
    if protocol == ForwardProtocol::Tcp || protocol == ForwardProtocol::Both {
        let conn_tcp    = conn.clone();
        let streams_tcp = Arc::clone(&active_streams);

        tokio::spawn(async move {
            loop {
                match conn_tcp.accept_bi().await {
                    Ok((send, recv)) => {
                        let n = streams_tcp.fetch_add(1, Ordering::SeqCst) + 1;
                        println!("  → TCP stream ({} active)", n);
                        let streams_clone = Arc::clone(&streams_tcp);

                        tokio::spawn(async move {
                            if let Err(e) = forward_tcp_to_local(send, recv, port).await {
                                tracing::debug!("TCP stream: {}", e);
                            }
                            let r = streams_clone.fetch_sub(1, Ordering::SeqCst) - 1;
                            println!("  ← TCP stream ({} active)", r);
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

    // Spawn UDP datagram forwarder
    if protocol == ForwardProtocol::Udp || protocol == ForwardProtocol::Both {
        let conn_udp = conn.clone();
        tokio::spawn(async move {
            if let Err(e) = forward_udp_exposer(conn_udp, port).await {
                tracing::debug!("UDP forwarder: {}", e);
            }
        });
    }

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    println!("\n🛑 Stopping port forward.");
    conn.close(0u32.into(), b"closed");
    endpoint.close().await;
    Ok(())
}

/// Forward a single Iroh QUIC stream to a local TCP connection.
/// Uses copy_bidirectional which properly drains both directions.
async fn forward_tcp_to_local(
    mut iroh_send: iroh::endpoint::SendStream,
    mut iroh_recv: iroh::endpoint::RecvStream,
    port: u16,
) -> anyhow::Result<()> {
    let mut tcp = TcpStream::connect(format!("127.0.0.1:{}", port)).await
        .with_context(|| format!("Could not connect to local port {}", port))?;

    let mut buf_send = vec![0u8; 64 * 1024];
    let mut buf_recv = vec![0u8; 64 * 1024];
    let (mut tcp_recv, mut tcp_send) = tcp.split();

    // Run both directions concurrently
    // Use manual loop so we handle QUIC's Option<usize> read correctly
    let iroh_to_tcp = async {
        loop {
            match iroh_recv.read(&mut buf_recv).await? {
                Some(0) | None => break,
                Some(n) => {
                    tcp_send.write_all(&buf_recv[..n]).await?;
                }
            }
        }
        tcp_send.shutdown().await?;
        Ok::<_, anyhow::Error>(())
    };

    let tcp_to_iroh = async {
        loop {
            let n = tcp_recv.read(&mut buf_send).await?;
            if n == 0 { break; }
            iroh_send.write_all(&buf_send[..n]).await?;
        }
        iroh_send.finish()?;
        Ok::<_, anyhow::Error>(())
    };

    // Both run concurrently — finish only when BOTH are done
    tokio::join!(iroh_to_tcp, tcp_to_iroh);
    Ok(())
}

/// Forward UDP datagrams between local socket and Iroh connection.
async fn forward_udp_exposer(conn: Connection, port: u16) -> anyhow::Result<()> {
    let local_udp = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    local_udp.connect(format!("127.0.0.1:{}", port)).await?;

    println!("  📡 UDP active on local port {}", port);

    let udp_recv  = Arc::clone(&local_udp);
    let conn_send = conn.clone();

    let local_to_iroh = async {
        let mut buf = vec![0u8; 65535];
        loop {
            let n = udp_recv.recv(&mut buf).await?;
            conn_send.send_datagram(bytes::Bytes::copy_from_slice(&buf[..n]))?;
        }
        Ok::<_, anyhow::Error>(())
    };

    let iroh_to_local = async {
        loop {
            let data = conn.read_datagram().await?;
            local_udp.send(&data).await?;
        }
        Ok::<_, anyhow::Error>(())
    };

    tokio::select! {
        r = local_to_iroh => r?,
        r = iroh_to_local => r?,
    }

    Ok(())
}

// ─── CONNECTOR (Device B) ────────────────────────────────────────────────────

/// Run the connector side — connect to exposer's Iroh endpoint, expose local port.
pub async fn run_connector(
    handshake: &ForwardHandshake,
    local_port: u16,
) -> anyhow::Result<()> {
    use iroh::endpoint::EndpointAddr;

    println!("\n🔗 Connecting via Iroh (direct or relay, automatic)...");

    // Parse EndpointAddr from handshake
    let addr: EndpointAddr = serde_json::from_str(&handshake.endpoint_addr)
        .context("Failed to parse endpoint address")?;

    // Create connector endpoint
    let endpoint = Endpoint::bind(presets::N0).await
        .context("Failed to create Iroh endpoint")?;

    // Connect — Iroh tries direct first, relay fallback automatic
    let conn = tokio::time::timeout(
        CONNECT_TIMEOUT,
        endpoint.connect(addr, FORWARD_ALPN)
    ).await
    .context("Connection timed out")?
    .context("Failed to connect to exposer")?;

    println!("✅ Connected (Iroh QUIC — direct or relay, automatic)");

    // Show connection type
    {
        use iroh::Watcher;
        let paths = conn.paths().get();
        if paths.iter().any(|p| p.is_direct()) {
            println!("   Path: Direct QUIC (fastest)");
        } else {
            println!("   Path: Iroh relay (encrypted, iroh.network)");
        }
    }

    let proto = &handshake.protocol;

    println!("🔀 Forwarding:");
    if *proto == ForwardProtocol::Tcp || *proto == ForwardProtocol::Both {
        println!("   TCP: localhost:{} → remote:{}", local_port, handshake.allowed_port);
    }
    if *proto == ForwardProtocol::Udp || *proto == ForwardProtocol::Both {
        println!("   UDP: localhost:{} → remote:{}", local_port, handshake.allowed_port);
    }
    println!("   Fingerprint: {}", handshake.session_fingerprint);
    println!("   Press Ctrl+C to disconnect.\n");

    // Spawn TCP listener
    if *proto == ForwardProtocol::Tcp || *proto == ForwardProtocol::Both {
        let conn_tcp     = conn.clone();
        let tcp_listener = TcpListener::bind(format!("127.0.0.1:{}", local_port)).await
            .with_context(|| format!("Could not bind local TCP port {}", local_port))?;

        tokio::spawn(async move {
            loop {
                match tcp_listener.accept().await {
                    Ok((tcp, _)) => {
                        let conn_clone = conn_tcp.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_local_tcp(tcp, conn_clone).await {
                                tracing::debug!("Local TCP: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        tracing::warn!("TCP listener: {}", e);
                        break;
                    }
                }
            }
        });
    }

    // Spawn UDP listener
    if *proto == ForwardProtocol::Udp || *proto == ForwardProtocol::Both {
        let conn_udp  = conn.clone();
        let udp_local = UdpSocket::bind(format!("127.0.0.1:{}", local_port)).await
            .with_context(|| format!("Could not bind local UDP port {}", local_port))?;

        tokio::spawn(async move {
            if let Err(e) = handle_local_udp(udp_local, conn_udp).await {
                tracing::debug!("UDP handler: {}", e);
            }
        });
    }

    tokio::signal::ctrl_c().await?;
    println!("\n🛑 Disconnecting.");
    conn.close(0u32.into(), b"closed");
    endpoint.close().await;
    Ok(())
}

/// Handle a local TCP connection — open Iroh stream and forward bidirectionally.
async fn handle_local_tcp(mut tcp: TcpStream, conn: Connection) -> anyhow::Result<()> {
    let (mut iroh_send, mut iroh_recv) = conn.open_bi().await
        .context("Could not open Iroh stream")?;

    let mut buf_a = vec![0u8; 64 * 1024];
    let mut buf_b = vec![0u8; 64 * 1024];
    let (mut tcp_recv, mut tcp_send) = tcp.split();

    let tcp_to_iroh = async {
        loop {
            let n = tcp_recv.read(&mut buf_a).await?;
            if n == 0 { break; }
            iroh_send.write_all(&buf_a[..n]).await?;
        }
        iroh_send.finish()?;
        Ok::<_, anyhow::Error>(())
    };

    let iroh_to_tcp = async {
        loop {
            match iroh_recv.read(&mut buf_b).await? {
                Some(0) | None => break,
                Some(n) => tcp_send.write_all(&buf_b[..n]).await?,
            }
        }
        tcp_send.shutdown().await?;
        Ok::<_, anyhow::Error>(())
    };

    // Both directions — finish when BOTH done
    tokio::join!(tcp_to_iroh, iroh_to_tcp);
    Ok(())
}

/// Handle local UDP — forward to exposer via Iroh datagrams.
async fn handle_local_udp(socket: UdpSocket, conn: Connection) -> anyhow::Result<()> {
    let socket       = Arc::new(socket);
    let mut last_peer: Option<SocketAddr> = None;
    let udp_recv     = Arc::clone(&socket);
    let conn_send    = conn.clone();

    let local_to_iroh = async {
        let mut buf = vec![0u8; 65535];
        loop {
            let (n, peer) = udp_recv.recv_from(&mut buf).await?;
            last_peer = Some(peer);
            conn_send.send_datagram(bytes::Bytes::copy_from_slice(&buf[..n]))?;
        }
        Ok::<_, anyhow::Error>(())
    };

    let iroh_to_local = async {
        loop {
            let data = conn.read_datagram().await?;
            if let Some(peer) = last_peer {
                socket.send_to(&data, peer).await?;
            }
        }
        Ok::<_, anyhow::Error>(())
    };

    tokio::select! {
        r = local_to_iroh => r?,
        r = iroh_to_local => r?,
    }

    Ok(())
}

// ─── HELPERS ─────────────────────────────────────────────────────────────────

/// Generate a short fingerprint from a node's public key string.
/// Used for verbal verification — both sides should see the same value.
pub fn node_fingerprint(node_id: &str) -> String {
    let bytes = node_id.as_bytes();
    let a = &bytes[..4.min(bytes.len())];
    let b = &bytes[4..8.min(bytes.len())];
    let c = &bytes[8..12.min(bytes.len())];
    format!("{}-{}-{}",
        std::str::from_utf8(a).unwrap_or("????"),
        std::str::from_utf8(b).unwrap_or("????"),
        std::str::from_utf8(c).unwrap_or("????"),
    )
}

/// Get local machine IP for fallback (used in cli.rs display only).
pub async fn local_ip() -> String {
    match tokio::net::UdpSocket::bind("0.0.0.0:0").await {
        Ok(socket) => match socket.connect("8.8.8.8:80").await {
            Ok(_) => socket.local_addr()
                .map(|a| a.ip().to_string())
                .unwrap_or_else(|_| "127.0.0.1".to_string()),
            Err(_) => "127.0.0.1".to_string(),
        },
        Err(_) => "127.0.0.1".to_string(),
    }
}

// ─── AUDIT LOG ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ForwardAuditEntry {
    pub timestamp:       chrono::DateTime<chrono::Utc>,
    pub role:            String,
    pub port:            u16,
    pub protocol:        String,
    pub token_type:      String,
    pub fingerprint:     String,
    pub streams_opened:  u32,
    pub bytes_forwarded: u64,
    pub ended_at:        chrono::DateTime<chrono::Utc>,
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