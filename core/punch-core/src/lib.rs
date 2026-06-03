//! punch-core — public API for Punch
//!
//! Option C: tracing only — no stdout in the library.
//! All events emitted via tracing::info!/warn!/debug!
//! The CLI (or any consumer) subscribes to tracing events.
//!
//! Function-based, minimal surface:
//!
//! ```rust
//! use punch_core as punch;
//!
//! punch::set_server("wss://your-server.com");
//!
//! let code = punch::generate().await?;
//! let conn = punch::connect("4829").await?;
//! punch::send_file("4829", "video.mp4").await?;
//! punch::receive_file("4829", "./downloads").await?;
//! punch::forward("4829", 8096).await?;
//! punch::forward_connect("4829").await?;
//! punch::pipe_send("4829").await?;
//! punch::pipe_recv("4829").await?;
//! ```

// ── Re-export internal modules so punch-cli can reach them
// without duplicating files. punch-cli replaces `mod signaling;`
// with `use punch_core::signaling;` etc.
pub mod internal;

// Re-export them so the CLI can still reach them easily
pub use internal::*;

// ── Public types consumers need ──────────────────────────────────────────────
pub use token::TokenType;
pub use transfer::TransferMeta;
pub use forward::{ForwardProtocol, ForwardHandshake};
pub use pipe::PipeHandshake;
pub use shell::ShellHandshake;

// ── Global server config ──────────────────────────────────────────────────────
use std::sync::OnceLock;
use std::sync::RwLock;

static SERVER: OnceLock<RwLock<String>> = OnceLock::new();

fn server_url() -> String {
    SERVER
        .get_or_init(|| RwLock::new("wss://129.159.21.6.nip.io".to_string()))
        .read()
        .map(|s| s.clone())
        .unwrap_or_else(|_| "wss://129.159.21.6.nip.io".to_string())
}

/// Override the default signalling server.
/// Call once before any other function.
pub fn set_server(url: &str) {
    let lock = SERVER.get_or_init(|| RwLock::new(url.to_string()));
    if let Ok(mut w) = lock.write() {
        *w = url.to_string();
    }
}

// ── Connection ────────────────────────────────────────────────────────────────

/// Generate a T-No (single-use) code and wait for a peer.
/// Returns the generated code.
pub async fn generate() -> anyhow::Result<String> {
    let t = token::Token::generate(None, false);
    token_store::store_token(&t).await?;
    tracing::info!(code = %t.code, token_type = "T-No", "Generated token");
    Ok(t.code)
}

/// Generate a Q-No (N-use) code and wait for a peer.
pub async fn generate_uses(n: u32) -> anyhow::Result<String> {
    let t = token::Token::generate(Some(n), false);
    token_store::store_token(&t).await?;
    tracing::info!(code = %t.code, uses = n, token_type = "Q-No", "Generated token");
    Ok(t.code)
}

/// Generate a P-No (permanent) code.
/// Caller must call verify_token() before first use.
pub async fn generate_permanent() -> anyhow::Result<String> {
    let t = token::Token::generate(None, true);
    token_store::store_token(&t).await?;
    tracing::info!(code = %t.code, token_type = "P-No", "Generated permanent token");
    Ok(t.code)
}

/// Connect to a peer using their code.
pub async fn connect(code: &str, log: bool) -> anyhow::Result<()> {
    tracing::info!(code, "Connecting to peer");
    let mut client = signaling::SignalingClient::connect(&server_url(), code).await?;
    let engine = punch::PunchEngine::new();
    engine.run_as_peer(&mut client, code, log).await?;
    Ok(())
}

// ── Token management ──────────────────────────────────────────────────────────

/// Verify a P-No token, enabling it for use.
pub async fn verify_token(code: &str) -> anyhow::Result<()> {
    token_store::verify_pno_token(code).await?;
    tracing::info!(code, "Token verified");
    Ok(())
}

/// Revoke a token immediately.
pub async fn revoke_token(code: &str) -> anyhow::Result<()> {
    token_store::revoke_token(code).await?;
    tracing::info!(code, "Token revoked");
    Ok(())
}

/// List all active stored tokens.
pub async fn list_tokens() -> Vec<token_store::StoredToken> {
    token_store::list_tokens().await
}

// ── File transfer ─────────────────────────────────────────────────────────────

/// Send a file to a peer identified by code.
pub async fn send_file(file_path: &str) -> anyhow::Result<String> {
    let path = std::path::PathBuf::from(file_path);
    anyhow::ensure!(path.exists(), "File not found: {}", file_path);

    let (meta, endpoint) = transfer::prepare_send(&path).await?;
    let t = token::Token::generate(None, false);

    tracing::info!(
        code = %t.code,
        file = %meta.filename,
        size = meta.total_size,
        "Sending file"
    );

    let mut client = signaling::SignalingClient::connect(&server_url(), &t.code).await?;
    client.wait_for_peer().await?;
    client.send_transfer_meta(&meta).await?;
    transfer::run_sender(&path, endpoint, &meta).await?;

    Ok(t.code)
}

/// Receive a file from a sender using their code.
/// Saves to dest_dir. Returns path of saved file.
pub async fn receive_file(code: &str, dest_dir: &str) -> anyhow::Result<std::path::PathBuf> {
    let dest = std::path::PathBuf::from(dest_dir);
    if !dest.exists() {
        tokio::fs::create_dir_all(&dest).await?;
    }

    let mut client = signaling::SignalingClient::connect(&server_url(), code).await?;
    let meta = client.wait_for_transfer_meta().await?;

    tracing::info!(
        file = %meta.filename,
        size = meta.total_size,
        "Receiving file"
    );

    let saved = transfer::run_receiver(&meta, &dest).await?;
    Ok(saved)
}

// ── Port forwarding ───────────────────────────────────────────────────────────

/// Expose a local TCP port to a peer.
/// Returns the generated code to share with the connector.
pub async fn forward(port: u16, udp: bool) -> anyhow::Result<String> {
    use std::sync::Arc;
    use std::sync::atomic::AtomicU32;

    let protocol = if udp {
        forward::ForwardProtocol::Both
    } else {
        forward::ForwardProtocol::Tcp
    };

    let t = token::Token::generate(None, false);
    token_store::store_token(&t).await?;

    let (handshake, endpoint) = forward::prepare_exposer(
        port, protocol, t.display_label()
    ).await?;

    tracing::info!(code = %t.code, port, "Exposing port");

    let mut client = signaling::SignalingClient::connect(&server_url(), &t.code).await?;
    client.wait_for_peer().await?;
    token_store::check_and_consume(&t.code).await?;
    client.send_forward_handshake(&handshake).await?;

    let streams = Arc::new(AtomicU32::new(0));
    forward::run_exposer(endpoint, &handshake, streams).await?;

    Ok(t.code)
}

/// Connect to a peer's forwarded port.
/// Returns the local port assigned.
pub async fn forward_connect(code: &str, local_port: Option<u16>) -> anyhow::Result<u16> {
    let mut client = signaling::SignalingClient::connect(&server_url(), code).await?;
    let handshake = client.wait_for_forward_handshake().await?;

    let local = match local_port {
        Some(p) => p,
        None => {
            let l = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            l.local_addr()?.port()
        }
    };

    tracing::info!(
        remote_port = handshake.allowed_port,
        local_port  = local,
        "Connecting to forwarded port"
    );

    forward::run_connector(&handshake, local).await?;
    Ok(local)
}

// ── Pipe ──────────────────────────────────────────────────────────────────────

/// Stream stdin to a peer. Returns code to share.
pub async fn pipe_send() -> anyhow::Result<String> {
    let t = token::Token::generate(None, false);
    let (handshake, endpoint) = pipe::prepare_pipe().await?;

    tracing::info!(code = %t.code, "Pipe send ready");

    let mut client = signaling::SignalingClient::connect(&server_url(), &t.code).await?;
    client.wait_for_peer().await?;
    client.send_pipe_handshake(&handshake).await?;
    pipe::run_pipe_sender(endpoint).await?;

    Ok(t.code)
}

/// Receive piped data from a peer and write to stdout.
pub async fn pipe_recv(code: &str) -> anyhow::Result<()> {
    let mut client = signaling::SignalingClient::connect(&server_url(), code).await?;
    let handshake = client.wait_for_pipe_handshake().await?;

    tracing::info!(code, "Pipe receive connecting");
    pipe::run_pipe_receiver(&handshake).await?;
    Ok(())
}
