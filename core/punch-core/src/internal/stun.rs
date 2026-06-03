use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};

const STUN_SERVERS: &[&str] = &[
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
    "stun.cloudflare.com:3478",
];

pub struct StunClient;
impl Default for StunClient {
    fn default() -> Self {
        Self::new()
    }
}
impl StunClient {
    pub fn new() -> Self {
        StunClient
    }

    /// Try each STUN server until one works.
    /// Returns None if all fail — caller falls back to relay.
    pub async fn discover(&self) -> Option<SocketAddr> {
        for server in STUN_SERVERS {
            match self.query(server).await {
                Ok(addr) => {
                    tracing::debug!("STUN via {}: {}", server, addr);
                    return Some(addr);
                }
                Err(e) => {
                    tracing::debug!("STUN {} failed: {}", server, e);
                }
            }
        }
        None
    }

    async fn query(&self, server: &str) -> anyhow::Result<SocketAddr> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;

        let server_addr: SocketAddr = tokio::net::lookup_host(server)
            .await?
            .next()
            .ok_or_else(|| anyhow::anyhow!("DNS failed for {}", server))?;

        // STUN Binding Request — 20 byte header, RFC 5389
        let mut req = [0u8; 20];
        req[0] = 0x00; req[1] = 0x01; // Binding Request
        req[2] = 0x00; req[3] = 0x00; // Length: 0 attributes
        req[4] = 0x21; req[5] = 0x12; req[6] = 0xA4; req[7] = 0x42; // Magic cookie
        for b in &mut req[8..20] { *b = rand::random(); } // Transaction ID

        socket.send_to(&req, server_addr).await?;

        let mut buf = [0u8; 512];
        let (n, _) = timeout(Duration::from_secs(3), socket.recv_from(&mut buf)).await??;

        self.parse_response(&buf[..n], &req[4..8])
    }

    fn parse_response(&self, data: &[u8], magic: &[u8]) -> anyhow::Result<SocketAddr> {
        if data.len() < 20 { anyhow::bail!("Response too short"); }
        // Must be a Binding Response (0x0101)
        if data[0] != 0x01 || data[1] != 0x01 { anyhow::bail!("Not a binding response"); }

        let msg_len = u16::from_be_bytes([data[2], data[3]]) as usize;
        let mut offset = 20;

        while offset + 4 <= 20 + msg_len {
            let attr_type = u16::from_be_bytes([data[offset], data[offset+1]]);
            let attr_len  = u16::from_be_bytes([data[offset+2], data[offset+3]]) as usize;
            offset += 4;

            if offset + attr_len > data.len() { break; }

            match attr_type {
                // XOR-MAPPED-ADDRESS (preferred)
                0x0020 if attr_len >= 8 => {
                    let xport = u16::from_be_bytes([data[offset+2], data[offset+3]])
                        ^ 0x2112;
                    let ip = [
                        data[offset+4] ^ magic[0],
                        data[offset+5] ^ magic[1],
                        data[offset+6] ^ magic[2],
                        data[offset+7] ^ magic[3],
                    ];
                    return Ok(SocketAddr::from((ip, xport)));
                }
                // MAPPED-ADDRESS (fallback)
                0x0001 if attr_len >= 8 => {
                    let port = u16::from_be_bytes([data[offset+2], data[offset+3]]);
                    let ip   = [data[offset+4], data[offset+5], data[offset+6], data[offset+7]];
                    return Ok(SocketAddr::from((ip, port)));
                }
                _ => {}
            }
            offset += (attr_len + 3) & !3; // pad to 4-byte boundary
        }

        anyhow::bail!("No mapped address in response")
    }
}