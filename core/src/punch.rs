use crate::signaling::SignalingClient;
use crate::stun::StunClient;
use crate::token::Token;
use crate::logger::{SessionLog, ConnectionType};

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;

const HOLE_PUNCH_TIMEOUT: Duration = Duration::from_secs(5);
const HOLE_PUNCH_ATTEMPTS: u32 = 10;

pub enum ConnectionResult {
    Direct(UdpSocket, SocketAddr),
    Relay,
}

pub struct PunchEngine;

impl PunchEngine {
    pub fn new() -> Self {
        PunchEngine
    }

    pub async fn run_as_host(
        &self,
        client: &mut SignalingClient,
        token: &Token,
        log_enabled: bool,
    ) -> anyhow::Result<()> {
        let session_start = chrono::Utc::now();

        let stun = StunClient::new();
        match stun.discover().await {
            Some(my_endpoint) => {
                println!("Public endpoint discovered: {}", my_endpoint);
                client.send_endpoint(&my_endpoint).await?;
                println!("Exchanging endpoints with peer...");
                let peer_endpoint = client.wait_for_peer_endpoint().await?;
                println!("Peer endpoint: {}", peer_endpoint);
                println!("\nPunching...");

                match self.attempt_hole_punch(my_endpoint, peer_endpoint).await {
                    Ok(ConnectionResult::Direct(socket, peer_addr)) => {
                        println!("Punched! Direct connection established.\n");
                        client.notify_handshake_complete().await?;
                        let session_id = crate::telemetry::emit_session_start(
                            &token.code,
                            ConnectionType::Direct,
                            "host",
                        );
                        self.run_session(
                            socket,
                            peer_addr,
                            log_enabled,
                            session_start,
                            &token.code,
                            ConnectionType::Direct,
                            &session_id,
                        )
                        .await?;
                        return Ok(());
                    }
                    _ => {
                        println!("Couldn't punch. Relaying encrypted traffic.\n");
                    }
                }
            }
            None => {
                println!("Couldn't punch. Relaying encrypted traffic.\n");
                client.signal_relay_fallback().await?;
            }
        }

        println!("Connected via encrypted relay.\n");
        let session_id =
            crate::telemetry::emit_session_start(&token.code, ConnectionType::Relay, "host");
        client
            .run_relay_session(log_enabled, session_start, &token.code, &session_id)
            .await?;
        Ok(())
    }

    pub async fn run_as_peer(
        &self,
        client: &mut SignalingClient,
        code: &str,
        log_enabled: bool,
    ) -> anyhow::Result<()> {
        let session_start = chrono::Utc::now();

        let stun = StunClient::new();
        match stun.discover().await {
            Some(my_endpoint) => {
                client.send_endpoint(&my_endpoint).await?;
                let peer_endpoint = client.wait_for_peer_endpoint().await?;
                println!("\nPunching...");

                match self.attempt_hole_punch(my_endpoint, peer_endpoint).await {
                    Ok(ConnectionResult::Direct(socket, peer_addr)) => {
                        println!("Punched! Direct connection established.\n");
                        client.notify_handshake_complete().await?;
                        let session_id = crate::telemetry::emit_session_start(
                            code,
                            ConnectionType::Direct,
                            "peer",
                        );
                        self.run_session(
                            socket,
                            peer_addr,
                            log_enabled,
                            session_start,
                            code,
                            ConnectionType::Direct,
                            &session_id,
                        )
                        .await?;
                        return Ok(());
                    }
                    _ => {
                        println!("Couldn't punch. Relaying encrypted traffic.\n");
                    }
                }
            }
            None => {
                println!("Couldn't punch. Relaying encrypted traffic.\n");
                client.signal_relay_fallback().await?;
            }
        }

        println!("Connected via encrypted relay.\n");
        let session_id =
            crate::telemetry::emit_session_start(code, ConnectionType::Relay, "peer");
        client
            .run_relay_session(log_enabled, session_start, code, &session_id)
            .await?;
        Ok(())
    }

    async fn attempt_hole_punch(
        &self,
        _my_addr: std::net::SocketAddr,
        peer_addr: std::net::SocketAddr,
    ) -> anyhow::Result<ConnectionResult> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;

        let punch_result = timeout(HOLE_PUNCH_TIMEOUT, async {
            for i in 0..HOLE_PUNCH_ATTEMPTS {
                socket.send_to(b"PUNCH", peer_addr).await?;
                tokio::time::sleep(Duration::from_millis(200)).await;

                let mut buf = [0u8; 16];
                match timeout(Duration::from_millis(100), socket.recv_from(&mut buf)).await {
                    Ok(Ok((_, addr))) if addr == peer_addr && &buf[..5] == b"PUNCH" => {
                        return Ok(true);
                    }
                    _ => {}
                }
                tracing::debug!("Punch attempt {}/{}", i + 1, HOLE_PUNCH_ATTEMPTS);
            }
            Ok::<bool, anyhow::Error>(false)
        })
        .await;

        match punch_result {
            Ok(Ok(true)) => Ok(ConnectionResult::Direct(socket, peer_addr)),
            _ => Ok(ConnectionResult::Relay),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_session(
        &self,
        socket: UdpSocket,
        peer_addr: SocketAddr,
        log_enabled: bool,
        session_start: chrono::DateTime<chrono::Utc>,
        code: &str,
        connection_type: ConnectionType,
        session_id: &str,
    ) -> anyhow::Result<()> {
        crate::telemetry::print_connect_session_help();

        let mut bytes_received: u64 = 0;
        let mut bytes_sent: u64 = 0;
        let mut buf = vec![0u8; 65536];

        let socket = Arc::new(socket);
        let send_sock = Arc::clone(&socket);
        let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        std::thread::spawn(move || {
            let mut line = String::new();
            let stdin = std::io::stdin();
            loop {
                line.clear();
                if stdin.read_line(&mut line).is_err() {
                    break;
                }
                if stdin_tx.send(line.clone()).is_err() {
                    break;
                }
            }
        });

        loop {
            tokio::select! {
                result = socket.as_ref().recv_from(&mut buf) => {
                    match result {
                        Ok((n, from)) if from == peer_addr => {
                            bytes_received += n as u64;
                            if let Ok(text) = std::str::from_utf8(&buf[..n]) {
                                if !text.trim().is_empty() {
                                    println!("< {}", text.trim_end());
                                }
                            } else {
                                tracing::debug!("Received {} bytes (binary)", n);
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            tracing::error!("Receive error: {}", e);
                            break;
                        }
                    }
                }
                line = stdin_rx.recv() => {
                    if let Some(l) = line {
                        let trimmed = l.trim_end().to_string();
                        if trimmed.is_empty() {
                            continue;
                        }
                        let payload = format!("{trimmed}\n");
                        if send_sock.send_to(payload.as_bytes(), peer_addr).await.is_ok() {
                            bytes_sent += payload.len() as u64;
                            println!("> {}", trimmed);
                        }
                    } else {
                        break;
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    println!("\nDisconnecting...");
                    break;
                }
            }
        }

        crate::telemetry::emit_session_end(
            session_id,
            code,
            connection_type,
            bytes_sent,
            bytes_received,
        );

        if log_enabled {
            let log = SessionLog {
                session_id: session_id.to_string(),
                token_code: code.to_string(),
                connection_type,
                started_at: session_start,
                ended_at: chrono::Utc::now(),
                bytes_sent,
                bytes_received,
            };
            crate::logger::write_log(log).await?;
            println!("Session logged to ~/.punch/logs/sessions.json");
        }

        println!("Disconnected.");
        Ok(())
    }
}
