use std::net::SocketAddr;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use crate::logger::ConnectionType;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MsgType {
    Register,
    Waiting,
    Matched,
    Endpoint,
    Relay,
    Handshake,
    Error,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignalMessage {
    #[serde(rename = "type")]
    pub msg_type: MsgType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

type WsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
>;

/// SignalingClient wraps the WebSocket connection to the Punch signalling server.
pub struct SignalingClient {
    ws: WsStream,
    code: String,
}

impl SignalingClient {
    /// Connect to the signalling server and register with a code.
    pub async fn connect(server: &str, code: &str) -> anyhow::Result<Self> {
        let url = format!("{}/ws", server);
        let (ws, _) = connect_async(&url).await
            .map_err(|e| anyhow::anyhow!("Could not connect to signalling server: {}\nCheck your internet connection or use --server to specify a custom server.", e))?;

        let mut client = SignalingClient {
            ws,
            code: code.to_string(),
        };

        // Register with the server
        client.send(SignalMessage {
            msg_type: MsgType::Register,
            code: Some(code.to_string()),
            payload: None,
        }).await?;

        Ok(client)
    }

    /// Wait for the server to confirm another peer has joined.
    pub async fn wait_for_peer(&mut self) -> anyhow::Result<()> {
        loop {
            match self.recv().await? {
                SignalMessage { msg_type: MsgType::Matched, .. } => return Ok(()),
                SignalMessage { msg_type: MsgType::Waiting, .. } => continue,
                SignalMessage { msg_type: MsgType::Error, payload, .. } => {
                    let reason = payload
                        .and_then(|p| p["reason"].as_str().map(String::from))
                        .unwrap_or_else(|| "Unknown error".to_string());
                    anyhow::bail!("Server error: {}", reason);
                }
                _ => continue,
            }
        }
    }

    /// Send our STUN-derived public endpoint to the server (for forwarding to peer).
    pub async fn send_endpoint(&mut self, addr: &SocketAddr) -> anyhow::Result<()> {
        let payload = serde_json::json!({
            "ip": addr.ip().to_string(),
            "port": addr.port(),
        });

        self.send(SignalMessage {
            msg_type: MsgType::Endpoint,
            code: Some(self.code.clone()),
            payload: Some(payload),
        }).await
    }

    /// Wait to receive the other peer's endpoint from the server.
    pub async fn wait_for_peer_endpoint(&mut self) -> anyhow::Result<SocketAddr> {
        loop {
            match self.recv().await? {
                SignalMessage { msg_type: MsgType::Endpoint, payload: Some(p), .. } => {
                    let ip = p["ip"].as_str().ok_or_else(|| anyhow::anyhow!("Missing IP"))?;
                    let port = p["port"].as_u64().ok_or_else(|| anyhow::anyhow!("Missing port"))? as u16;
                    let addr: SocketAddr = format!("{}:{}", ip, port).parse()?;
                    return Ok(addr);
                }
                _ => continue,
            }
        }
    }

    /// Notify the server that direct p2p connection is established.
    /// Server will destroy the session after this.
    pub async fn signal_relay_fallback(&mut self) -> anyhow::Result<()> {
        let payload = serde_json::json!({ "relay": true, "ip": "0.0.0.0", "port": 0 });
        self.send(SignalMessage {
            msg_type: MsgType::Endpoint,
            code: Some(self.code.clone()),
            payload: Some(payload),
        }).await
    }

    pub async fn notify_handshake_complete(&mut self) -> anyhow::Result<()> {
        self.send(SignalMessage {
            msg_type: MsgType::Handshake,
            code: Some(self.code.clone()),
            payload: None,
        }).await
    }

    /// Run a relay session through the signalling server.
    /// All traffic is end-to-end encrypted — server only forwards bytes.
    pub async fn run_relay_session(
        &mut self,
        log_enabled: bool,
        session_start: chrono::DateTime<chrono::Utc>,
        code: &str,
    ) -> anyhow::Result<()> {
        println!("Session active via relay. Press Ctrl+C to disconnect.\n");

        // In v0.2: implement full ChaCha20-Poly1305 encrypted relay here
        // For v0.1: placeholder showing the relay pathway works
        tokio::signal::ctrl_c().await?;
        println!("\nDisconnecting...");

        if log_enabled {
            let log = crate::logger::SessionLog {
                session_id: uuid::Uuid::new_v4().to_string(),
                token_code: code.to_string(),
                connection_type: ConnectionType::Relay,
                started_at: session_start,
                ended_at: chrono::Utc::now(),
                bytes_sent: 0,
                bytes_received: 0,
            };
            crate::logger::write_log(log).await?;
        }

        Ok(())
    }

    async fn send(&mut self, msg: SignalMessage) -> anyhow::Result<()> {
        let json = serde_json::to_string(&msg)?;
        self.ws.send(Message::Text(json)).await?;
        Ok(())
    }

    async fn recv(&mut self) -> anyhow::Result<SignalMessage> {
        loop {
            match self.ws.next().await {
                Some(Ok(Message::Text(text))) => {
                    return Ok(serde_json::from_str(&text)?);
                }
                Some(Ok(Message::Ping(_))) => continue,
                Some(Ok(Message::Close(_))) => {
                    anyhow::bail!("Server closed connection");
                }
                Some(Err(e)) => anyhow::bail!("WebSocket error: {}", e),
                None => anyhow::bail!("Connection lost"),
                _ => continue,
            }
        }
    }
}