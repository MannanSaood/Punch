//! punch pipe — stream stdin/stdout between two devices
//! pipe: stdin → QUIC stream → stdout (one direction)

use anyhow::Context;
use iroh::endpoint::Endpoint;
use iroh::{EndpointAddr, Watcher};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const PIPE_ALPN: &[u8] = b"punch/pipe/1";

/// Shared over signalling so the receiver can connect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipeHandshake {
    pub endpoint_addr: String,
}

// ─── PIPE SENDER (Device A) ──────────────────────────────────────────────────

/// Bind Iroh endpoint and return handshake to share via signalling.
pub async fn prepare_pipe() -> anyhow::Result<(PipeHandshake, Endpoint)> {
    let endpoint = Endpoint::builder()
        .alpns(vec![PIPE_ALPN.to_vec()])
        .bind()
        .await
        .context("Failed to bind Iroh endpoint")?;

    // In Iroh 0.96+, the EndpointAddr is never null, so we just get() it directly
    // instead of waiting for it to be initialized.
    let addr = endpoint.watch_addr().get();

    let handshake = PipeHandshake {
        endpoint_addr: serde_json::to_string(&addr)?,
    };

    Ok((handshake, endpoint))
}

/// Sender side — read stdin, write to QUIC stream.
pub async fn run_pipe_sender(endpoint: Endpoint) -> anyhow::Result<()> {
    eprintln!("Waiting for receiver...");

    let conn = tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            match endpoint.accept().await {
                Some(inc) => match inc.await {
                    Ok(c)  => return Ok(c),
                    Err(e) => tracing::warn!("Accept: {}", e),
                },
                None => anyhow::bail!("Endpoint closed"),
            }
        }
    }).await.context("Timed out")??;

    let (mut send, _) = conn.open_bi().await?;

    eprintln!("Connected. Streaming stdin...\n");

    let mut stdin = tokio::io::stdin();
    let mut buf   = vec![0u8; 16 * 1024];

    loop {
        let n = stdin.read(&mut buf).await?;
        if n == 0 { break; }
        send.write_all(&buf[..n]).await?;
    }

    send.finish()?;
    conn.close(0u32.into(), b"done");
    endpoint.close().await;
    Ok(())
}

// ─── PIPE RECEIVER (Device B) ────────────────────────────────────────────────

/// Receiver side — connect to sender, write stream to stdout.
pub async fn run_pipe_receiver(handshake: &PipeHandshake) -> anyhow::Result<()> {
    let addr: EndpointAddr = serde_json::from_str(&handshake.endpoint_addr)?;

    let endpoint = Endpoint::bind().await?;

    let conn = tokio::time::timeout(
        Duration::from_secs(30),
        endpoint.connect(addr, PIPE_ALPN)
    ).await.context("Connection timed out")?
     .context("Failed to connect")?;

    let (_, mut recv) = conn.accept_bi().await?;

    let mut stdout = tokio::io::stdout();
    let mut buf    = vec![0u8; 16 * 1024];

    loop {
        match recv.read(&mut buf).await? {
            Some(0) | None => break,
            Some(n) => stdout.write_all(&buf[..n]).await?,
        }
    }

    stdout.flush().await?;
    conn.close(0u32.into(), b"done");
    endpoint.close().await;
    Ok(())
}