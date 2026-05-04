use std::net::SocketAddr;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use x25519_dalek::PublicKey;
use crate::crypto::{SessionCipher, SessionKeypair};
use crate::transfer::TransferMeta;
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
    PublicKey,  // new: for key exchange
    Transfer,   // file transfer metadata
    Forward,    // port forward handshake
    Shell,      // shell session handshake
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

pub struct SignalingClient {
    ws: WsStream,
    code: String,
}

impl SignalingClient {
    pub async fn connect(server: &str, code: &str) -> anyhow::Result<Self> {
        let url = format!("{}/ws", server);
        let (ws, _) = connect_async(&url).await
            .map_err(|e| anyhow::anyhow!(
                "Could not connect to signalling server: {}\nCheck your internet connection or use --server to specify a custom server.", e
            ))?;

        let mut client = SignalingClient { ws, code: code.to_string() };

        client.send(SignalMessage {
            msg_type: MsgType::Register,
            code: Some(code.to_string()),
            payload: None,
        }).await?;

        Ok(client)
    }

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

    /// Full encrypted relay session.
    /// Step 1: X25519 key exchange through server (server sees public keys only)
    /// Step 2: Derive shared secret independently on both sides
    /// Step 3: All relay traffic encrypted with ChaCha20-Poly1305
    pub async fn run_relay_session(
        &mut self,
        log_enabled: bool,
        session_start: chrono::DateTime<chrono::Utc>,
        code: &str,
    ) -> anyhow::Result<()> {

        // --- Step 1: Key Exchange ---
        let keypair = SessionKeypair::generate();
        let my_public_bytes = keypair.public_bytes();

        // Send our public key to the peer via server
        self.send(SignalMessage {
            msg_type: MsgType::PublicKey,
            code: Some(self.code.clone()),
            payload: Some(serde_json::json!({
                "key": base64_encode(&my_public_bytes)
            })),
        }).await?;

        // Wait for peer's public key
        let peer_public_bytes = loop {
            match self.recv().await? {
                SignalMessage { msg_type: MsgType::PublicKey, payload: Some(p), .. } => {
                    let key_b64 = p["key"].as_str()
                        .ok_or_else(|| anyhow::anyhow!("Missing key"))?;
                    let bytes = base64_decode(key_b64)?;
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    break arr;
                }
                _ => continue,
            }
        };

        // --- Step 2: Derive shared secret ---
        // Both peers independently compute the same secret.
        // Server only saw two public keys — cannot derive the secret.
        let peer_public = PublicKey::from(peer_public_bytes);
        let cipher = keypair.derive_shared_secret(peer_public);

        println!("🔑 Keys exchanged. End-to-end encrypted.\n");

        // --- Step 3: Encrypted relay loop ---
        self.encrypted_relay_loop(cipher, log_enabled, session_start, code).await
    }

    async fn encrypted_relay_loop(
        &mut self,
        cipher: SessionCipher,
        log_enabled: bool,
        session_start: chrono::DateTime<chrono::Utc>,
        code: &str,
    ) -> anyhow::Result<()> {
        println!("Session active via encrypted relay. Press Ctrl+C to disconnect.\n");

        let mut bytes_sent: u64 = 0;
        let mut bytes_received: u64 = 0;

        loop {
            tokio::select! {
                msg = self.recv() => {
                    match msg {
                        Ok(SignalMessage { msg_type: MsgType::Relay, payload: Some(p), .. }) => {
                            if let Some(data_b64) = p["data"].as_str() {
                                if let Ok(encrypted) = base64_decode(data_b64) {
                                    match cipher.decrypt(&encrypted) {
                                        Ok(plain) => {
                                            bytes_received += plain.len() as u64;
                                            tracing::debug!("Received {} bytes (decrypted)", plain.len());
                                        }
                                        Err(e) => {
                                            tracing::warn!("Decryption failed: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        Ok(SignalMessage { msg_type: MsgType::Error, .. }) => {
                            println!("\nPeer disconnected.");
                            break;
                        }
                        Err(e) => {
                            tracing::debug!("Connection error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    println!("\nDisconnecting...");
                    break;
                }
            }
        }

        if log_enabled {
            let log = crate::logger::SessionLog {
                session_id: uuid::Uuid::new_v4().to_string(),
                token_code: code.to_string(),
                connection_type: ConnectionType::Relay,
                started_at: session_start,
                ended_at: chrono::Utc::now(),
                bytes_sent,
                bytes_received,
            };
            crate::logger::write_log(log).await?;
        }

        Ok(())
    }

    /// Send shell handshake to client.
    pub async fn send_shell_handshake(&mut self, h: &crate::shell::ShellHandshake) -> anyhow::Result<()> {
        let payload = serde_json::to_value(h)?;
        self.send(SignalMessage {
            msg_type: MsgType::Shell,
            code: Some(self.code.clone()),
            payload: Some(payload),
        }).await
    }

    /// Wait to receive shell handshake from host.
    pub async fn wait_for_shell_handshake(&mut self) -> anyhow::Result<crate::shell::ShellHandshake> {
        loop {
            match self.recv().await? {
                SignalMessage { msg_type: MsgType::Shell, payload: Some(p), .. } => {
                    return Ok(serde_json::from_value(p)?);
                }
                _ => continue,
            }
        }
    }

    /// Send port forward handshake to connector.
    pub async fn send_forward_handshake(&mut self, handshake: &crate::forward::ForwardHandshake) -> anyhow::Result<()> {
        let payload = serde_json::to_value(handshake)?;
        self.send(SignalMessage {
            msg_type: MsgType::Forward,
            code: Some(self.code.clone()),
            payload: Some(payload),
        }).await
    }

    /// Wait to receive port forward handshake from exposer.
    pub async fn wait_for_forward_handshake(&mut self) -> anyhow::Result<crate::forward::ForwardHandshake> {
        loop {
            match self.recv().await? {
                SignalMessage { msg_type: MsgType::Forward, payload: Some(p), .. } => {
                    return Ok(serde_json::from_value(p)?);
                }
                _ => continue,
            }
        }
    }

    /// Send Quinn address to connector so they know where to connect.
    pub async fn send_quinn_addr(&mut self, addr: &str) -> anyhow::Result<()> {
        let payload = serde_json::json!({ "quinn_addr": addr });
        self.send(SignalMessage {
            msg_type: MsgType::Endpoint,
            code: Some(self.code.clone()),
            payload: Some(payload),
        }).await
    }

    /// Wait to receive Quinn address from exposer.
    pub async fn wait_for_quinn_addr(&mut self) -> anyhow::Result<String> {
        loop {
            match self.recv().await? {
                SignalMessage { msg_type: MsgType::Endpoint, payload: Some(p), .. } => {
                    if let Some(addr) = p["quinn_addr"].as_str() {
                        return Ok(addr.to_string());
                    }
                }
                _ => continue,
            }
        }
    }

        /// Send file transfer metadata to the receiver via signalling server.
    pub async fn send_transfer_meta(&mut self, meta: &TransferMeta) -> anyhow::Result<()> {
        let payload = serde_json::to_value(meta)?;
        self.send(SignalMessage {
            msg_type: MsgType::Transfer,
            code: Some(self.code.clone()),
            payload: Some(payload),
        }).await
    }

    /// Wait to receive file transfer metadata from the sender.
    pub async fn wait_for_transfer_meta(&mut self) -> anyhow::Result<TransferMeta> {
        loop {
            match self.recv().await? {
                SignalMessage { msg_type: MsgType::Transfer, payload: Some(p), .. } => {
                    let meta: TransferMeta = serde_json::from_value(p)?;
                    return Ok(meta);
                }
                _ => continue,
            }
        }
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
                Some(Ok(Message::Close(_))) => anyhow::bail!("Server closed connection"),
                Some(Err(e)) => anyhow::bail!("WebSocket error: {}", e),
                None => anyhow::bail!("Connection lost"),
                _ => continue,
            }
        }
    }
}

// Minimal base64 helpers to avoid adding another dependency
fn base64_encode(data: &[u8]) -> String {
    use std::fmt::Write;
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as usize;
        let b1 = if i+1 < data.len() { data[i+1] as usize } else { 0 };
        let b2 = if i+2 < data.len() { data[i+2] as usize } else { 0 };
        write!(out, "{}{}{}{}", 
            CHARS[b0 >> 2] as char,
            CHARS[((b0 & 3) << 4) | (b1 >> 4)] as char,
            if i+1 < data.len() { CHARS[((b1 & 15) << 2) | (b2 >> 6)] as char } else { '=' },
            if i+2 < data.len() { CHARS[b2 & 63] as char } else { '=' },
        ).unwrap();
        i += 3;
    }
    out
}

fn base64_decode(s: &str) -> anyhow::Result<Vec<u8>> {
    let s = s.replace('=', "");
    let mut out = Vec::new();
    let chars: Vec<u8> = s.bytes().map(|c| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => 255,
    }).collect();

    let mut i = 0;
    while i + 1 < chars.len() {
        let b0 = chars[i];
        let b1 = chars[i+1];
        out.push((b0 << 2) | (b1 >> 4));
        if i+2 < chars.len() {
            let b2 = chars[i+2];
            out.push((b1 << 4) | (b2 >> 2));
            if i+3 < chars.len() {
                let b3 = chars[i+3];
                out.push((b2 << 6) | b3);
            }
        }
        i += 4;
    }
    Ok(out)
}