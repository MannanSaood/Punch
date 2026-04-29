use crate::signaling::SignalingClient;
use crate::stun::StunClient;
use crate::token::Token;
use crate::logger::{SessionLog, ConnectionType};

use std::net::SocketAddr;
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

        // Try STUN — if it fails, go straight to relay (don't crash)
        let stun = StunClient::new();
        match stun.discover().await {
            Some(my_endpoint) => {
                println!("🌐 Public endpoint discovered: {}", my_endpoint);
                client.send_endpoint(&my_endpoint).await?;
                println!("Exchanging endpoints with peer...");
                let peer_endpoint = client.wait_for_peer_endpoint().await?;
                println!("🌐 Peer endpoint: {}", peer_endpoint);
                println!("\nPunching...");

                match self.attempt_hole_punch(my_endpoint, peer_endpoint).await {
                    Ok(ConnectionResult::Direct(socket, peer_addr)) => {
                        println!("✅ Punched! Direct connection established.\n");
                        client.notify_handshake_complete().await?;
                        self.run_session(socket, peer_addr, log_enabled, session_start, &token.code, ConnectionType::Direct).await?;
                        return Ok(());
                    }
                    _ => {
                        println!("❌ Couldn't punch. Relaying encrypted traffic.\n");
                    }
                }
            }
            None => {
                println!("❌ Couldn't punch. Relaying encrypted traffic.\n");
                // Signal to peer that we're going relay so they don't wait for endpoint
                client.signal_relay_fallback().await?;
            }
        }

        println!("🔒 Connected via encrypted relay.\n");
        client.run_relay_session(log_enabled, session_start, &token.code).await?;
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
                        println!("✅ Punched! Direct connection established.\n");
                        client.notify_handshake_complete().await?;
                        self.run_session(socket, peer_addr, log_enabled, session_start, code, ConnectionType::Direct).await?;
                        return Ok(());
                    }
                    _ => {
                        println!("❌ Couldn't punch. Relaying encrypted traffic.\n");
                    }
                }
            }
            None => {
                println!("❌ Couldn't punch. Relaying encrypted traffic.\n");
                client.signal_relay_fallback().await?;
            }
        }

        println!("🔒 Connected via encrypted relay.\n");
        client.run_relay_session(log_enabled, session_start, code).await?;
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
                    Ok(Ok((_, addr))) if addr == peer_addr => {
                        if &buf[..5] == b"PUNCH" {
                            return Ok(true);
                        }
                    }
                    _ => {}
                }
                tracing::debug!("Punch attempt {}/{}", i + 1, HOLE_PUNCH_ATTEMPTS);
            }
            Ok::<bool, anyhow::Error>(false)
        }).await;

        match punch_result {
            Ok(Ok(true)) => Ok(ConnectionResult::Direct(socket, peer_addr)),
            _ => Ok(ConnectionResult::Relay),
        }
    }

    async fn run_session(
        &self,
        socket: UdpSocket,
        peer_addr: SocketAddr,
        log_enabled: bool,
        session_start: chrono::DateTime<chrono::Utc>,
        code: &str,
        connection_type: ConnectionType,
    ) -> anyhow::Result<()> {
        println!("Session active. Press Ctrl+C to disconnect.\n");

        let mut bytes_received: u64 = 0;
        let bytes_sent: u64 = 0;
        let mut buf = vec![0u8; 65536];

        loop {
            tokio::select! {
                result = socket.recv_from(&mut buf) => {
                    match result {
                        Ok((n, _)) => {
                            bytes_received += n as u64;
                            tracing::debug!("Received {} bytes", n);
                        }
                        Err(e) => {
                            tracing::error!("Receive error: {}", e);
                            break;
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    println!("\nDisconnecting...");
                    break;
                }
            }
        }

        if log_enabled {
            let log = SessionLog {
                session_id: uuid::Uuid::new_v4().to_string(),
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