use crate::token::{Token, TokenType};
use crate::token_store::{self, StoredTokenType};
use crate::signaling::SignalingClient;
use crate::punch::PunchEngine;

/// Handle `punch generate`
pub async fn generate(
    server: String,
    uses: Option<u32>,
    permanent: bool,
    log_enabled: bool,
) -> anyhow::Result<()> {
    let token = Token::generate(uses, permanent);

    if token.token_type == TokenType::PNo {
        println!("⚠️  You are creating a PERMANENT access token.");
        println!("   Anyone with this code will always be able to connect.");
        println!("   You will need to verify it before first use.");
        println!("\n   Are you sure? (yes/no): ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() != "yes" {
            println!("Cancelled.");
            return Ok(());
        }
    }

    token_store::store_token(&token).await?;

    println!("\n{}: {}", token.display_label(), token.code);
    println!("Type: {}", token.token_type);

    if token.token_type == TokenType::PNo {
        println!("\n⚠️  Run this to verify before first use:");
        println!("   punch verify {}", token.code);
    }

    println!("\nWaiting for peer...\n");

    let mut client = SignalingClient::connect(&server, &token.code).await?;
    client.wait_for_peer().await?;

    match token_store::check_and_consume(&token.code).await {
        Ok(()) => {}
        Err(e) => {
            println!("❌ Connection rejected: {}", e);
            return Ok(());
        }
    }

    let engine = PunchEngine::new();
    engine.run_as_host(&mut client, &token, log_enabled).await?;

    Ok(())
}

/// Handle `punch listen <code>` — reconnect on existing token without generating a new one
pub async fn listen(
    server: String,
    code: String,
    log_enabled: bool,
) -> anyhow::Result<()> {
    let tokens = token_store::list_tokens().await;
    let stored = tokens.iter().find(|t| t.code == code)
        .ok_or_else(|| anyhow::anyhow!(
            "Token {} not found. Generate one first with: punch generate --uses N", code
        ))?;

    if !stored.is_valid() {
        anyhow::bail!("Token {} has expired and can no longer be used.", code);
    }

    let type_label = match &stored.token_type {
        StoredTokenType::TNo => "T-No",
        StoredTokenType::QNo { .. } => "Q-No",
        StoredTokenType::PNo { .. } => "P-No",
    };

    println!("{}: {}", type_label, code);
    println!("Status: {}\n", stored.status());
    println!("Waiting for peer...\n");

    let mut client = SignalingClient::connect(&server, &code).await?;
    client.wait_for_peer().await?;

    // Only consume the token after peer actually connects
    match token_store::check_and_consume(&code).await {
        Ok(()) => {}
        Err(e) => {
            println!("❌ Connection rejected: {}", e);
            return Ok(());
        }
    }

    let engine = PunchEngine::new();
    let token = Token { code: code.clone(), token_type: TokenType::TNo };
    engine.run_as_host(&mut client, &token, log_enabled).await?;

    Ok(())
}

/// Handle `punch connect <code>`
pub async fn connect(
    server: String,
    code: String,
    log_enabled: bool,
) -> anyhow::Result<()> {
    println!("Connecting with code: {}\n", code);

    let mut client = SignalingClient::connect(&server, &code).await?;

    let engine = PunchEngine::new();
    engine.run_as_peer(&mut client, &code, log_enabled).await?;

    Ok(())
}

/// Handle `punch verify <code>`
pub async fn verify(code: String) -> anyhow::Result<()> {
    println!("Verifying permanent token: {}", code);
    println!("This will allow permanent access to your device.");
    println!("Confirm? (yes/no): ");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() != "yes" {
        println!("Cancelled.");
        return Ok(());
    }

    token_store::verify_pno_token(&code).await
}

/// Handle `punch revoke <code>`
pub async fn revoke(code: String) -> anyhow::Result<()> {
    token_store::revoke_token(&code).await
}

/// Handle `punch tokens`
pub async fn tokens() -> anyhow::Result<()> {
    let tokens = token_store::list_tokens().await;

    if tokens.is_empty() {
        println!("No active tokens. Generate one with: punch generate");
        return Ok(());
    }

    println!("\nActive tokens:\n");
    println!("{:<8} {:<12} {:<30} {}", "Code", "Type", "Status", "Created");
    println!("{}", "─".repeat(72));

    for t in tokens {
        let type_label = match &t.token_type {
            StoredTokenType::TNo => "T-No".to_string(),
            StoredTokenType::QNo { .. } => "Q-No".to_string(),
            StoredTokenType::PNo { .. } => "P-No".to_string(),
        };
        println!(
            "{:<8} {:<12} {:<30} {}",
            t.code,
            type_label,
            t.status(),
            t.created_at.format("%Y-%m-%d %H:%M")
        );
    }
    println!();
    Ok(())
}

/// Handle `punch dashboard`
pub async fn dashboard() -> anyhow::Result<()> {
    // Open browser automatically
    let _ = open_browser("http://localhost:7777");
    crate::dashboard_server::serve().await
}

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/c", "start", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

/// Handle `punch send <file> <code>`
pub async fn send(
    server: String,
    file_path: String,
    log_enabled: bool,
) -> anyhow::Result<()> {
    let path = std::path::PathBuf::from(&file_path);
    if !path.exists() {
        anyhow::bail!("File not found: {}", file_path);
    }

    // Prepare file — creates Iroh endpoint, gets NodeAddr automatically
    // Iroh handles STUN, hole punching, and relay fallback internally
    let (meta, endpoint) = crate::transfer::prepare_send(&path).await?;

    // Show sender-side safety info + fingerprint
    crate::safety::display_sender_info(
        &meta.filename,
        meta.total_size,
        &meta.file_checksum,
    );

    // Warn about resume availability based on token type
    warn_resume_availability(&meta).await;

    // Generate a T-No code for this transfer
    let token = crate::token::Token::generate(None, false);
    println!("
T-No: {}", token.code);
    println!("Share this code with the receiver.
");
    println!("Waiting for receiver...
");

    // Signal via server — share file metadata including Iroh NodeAddr
    let mut client = SignalingClient::connect(&server, &token.code).await?;
    client.wait_for_peer().await?;
    client.send_transfer_meta(&meta).await?;

    // Serve chunks over Iroh QUIC — direct or relay, automatic
    crate::transfer::run_sender(&path, endpoint, &meta).await?;

    Ok(())
}

/// Handle `punch receive <code> --dest <path>`
pub async fn receive(
    server: String,
    code: String,
    dest: String,
) -> anyhow::Result<()> {
    let dest_dir = std::path::PathBuf::from(&dest);
    if !dest_dir.exists() {
        tokio::fs::create_dir_all(&dest_dir).await?;
    }

    println!("Connecting with code: {}\n", code);

    // Connect and get file metadata from sender
    let mut client = SignalingClient::connect(&server, &code).await?;
    let meta = client.wait_for_transfer_meta().await?;

    println!("📁 Incoming: {} ({} MB)",
        meta.filename,
        meta.total_size / (1024 * 1024)
    );
    println!("📦 {} chunks × {}MB, {} parallel streams\n",
        meta.chunk_count,
        meta.chunk_size / (1024 * 1024),
        meta.parallel_streams
    );

    // Connect directly to sender — bypasses server entirely
    crate::transfer::run_receiver(&meta, &dest_dir).await?;

    Ok(())
}


/// Warn the user about resume availability based on active token state.
/// T-No = no resume possible at all.
/// Q-No with 1 use = last chance, no resume if it drops.
/// Q-No with 2+ uses = resume costs another use.
/// P-No = unlimited resume, no warning needed.
async fn warn_resume_availability(meta: &crate::transfer::TransferMeta) {
    use crate::token_store::{list_tokens, StoredTokenType};

    // Large file threshold — warn more aggressively above 500MB
    let size_mb = meta.total_size / (1024 * 1024);
    let is_large = size_mb > 500;

    let tokens = list_tokens().await;

    // Find if current transfer was initiated from a stored token
    // For T-No (not stored) — always warn no resume
    let stored = tokens.iter().find(|t| {
        // Heuristic: most recently created token is likely this one
        true // we check all, show most relevant warning
    });

    // T-No or no stored token
    if tokens.is_empty() {
        println!("⚠️  T-No token — no resume possible if transfer drops.");
        if is_large {
            println!("   ⚠️  File is {}MB. Consider using Q-No or P-No for large transfers:", size_mb);
            println!("   punch send {} --uses 3", meta.filename);
        }
        println!();
        return;
    }

    // Check Q-No remaining uses
    for token in &tokens {
        match &token.token_type {
            StoredTokenType::QNo { remaining } => {
                if *remaining == 1 {
                    println!("⚠️  This is your LAST use of Q-No token {}.", token.code);
                    println!("   If the transfer drops mid-way, you cannot resume.");
                    println!("   The token will be consumed whether transfer completes or not.");
                    if is_large {
                        println!("   ⚠️  File is {}MB — consider generating a P-No token for reliable large transfers.", size_mb);
                    }
                    println!();
                } else if *remaining <= 3 {
                    println!("ℹ️  Q-No token: {} uses remaining.", remaining);
                    println!("   Resume will cost 1 additional use if transfer drops.");
                    println!();
                }
            }
            StoredTokenType::PNo { verified } => {
                if !verified {
                    println!("⚠️  P-No token not verified — connection will be rejected.");
                    println!("   Run: punch verify <code>");
                    println!();
                }
                // Verified P-No = safest, no warning needed
            }
            StoredTokenType::TNo => {
                println!("⚠️  T-No token — no resume possible if transfer drops.");
                if is_large {
                    println!("   ⚠️  File is {}MB. T-No is risky for large files.", size_mb);
                }
                println!();
            }
        }
    }
}

/// Handle `punch forward expose <port>`
pub async fn forward_expose(
    server: String,
    port: u16,
    udp: bool,
    token_uses: Option<u32>,
    permanent: bool,
) -> anyhow::Result<()> {
    use crate::forward::{ForwardProtocol, prepare_exposer, run_exposer};
    use std::sync::Arc;
    use std::sync::atomic::AtomicU32;

    let protocol = if udp { ForwardProtocol::Both } else { ForwardProtocol::Tcp };

    // Generate token
    let token = crate::token::Token::generate(token_uses, permanent);
    crate::token_store::store_token(&token).await?;

    println!("\n{}: {}", token.display_label(), token.code);
    println!("Type:      {}", token.token_type);
    println!("Protocol:  {}", protocol);
    println!("Port:      {}\n", port);

    if token.token_type == crate::token::TokenType::PNo {
        println!("⚠️  Run: punch verify {} before first use\n", token.code);
    }

    // Prepare Iroh endpoint — handles STUN, hole punch, relay automatically
    let (handshake, iroh_endpoint) = prepare_exposer(
        port, protocol, &token.display_label()
    ).await?;

    println!("\nWaiting for connector...\n");

    // Signal via server — send full handshake including EndpointAddr
    let mut client = SignalingClient::connect(&server, &token.code).await?;
    client.wait_for_peer().await?;

    // Enforce token policy
    match crate::token_store::check_and_consume(&token.code).await {
        Ok(()) => {}
        Err(e) => {
            println!("❌ Connection rejected: {}", e);
            return Ok(());
        }
    }

    // Send full handshake — includes EndpointAddr, no separate quinn_addr needed
    client.send_forward_handshake(&handshake).await?;

    let active_streams = Arc::new(AtomicU32::new(0));
    run_exposer(iroh_endpoint, &handshake, active_streams).await?;

    Ok(())
}

/// Handle `punch forward connect <code>`
pub async fn forward_connect(
    server: String,
    code: String,
    local_port: Option<u16>,
    _udp: bool,
) -> anyhow::Result<()> {
    use crate::forward::run_connector;
    use std::io::Write;

    println!("Connecting with code: {}\n", code);

    let mut client = SignalingClient::connect(&server, &code).await?;

    // Get full handshake from exposer — contains EndpointAddr
    let handshake = client.wait_for_forward_handshake().await?;

    // Show consent prompt
    println!("─────────────────────────────────────────");
    println!("  🔀 Incoming port forward request");
    println!("─────────────────────────────────────────");
    println!("  Remote port:  {} ({})", handshake.allowed_port, handshake.protocol);
    println!("  Token type:   {}", handshake.token_type);
    println!("  Fingerprint:  {}", handshake.session_fingerprint);
    println!("─────────────────────────────────────────");
    println!();
    println!("  Verify fingerprint with exposer before accepting.");
    print!("  Connect? (yes/no): ");
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if input.trim().to_lowercase() != "yes" {
        println!("Cancelled.");
        return Ok(());
    }

    // Auto-assign local port if not specified
    let local = match local_port {
        Some(p) => p,
        None    => find_free_port().await?,
    };

    println!();

    // Write audit log
    crate::forward::write_forward_log(crate::forward::ForwardAuditEntry {
        timestamp:       chrono::Utc::now(),
        role:            "connector".to_string(),
        port:            handshake.allowed_port,
        protocol:        handshake.protocol.to_string(),
        token_type:      handshake.token_type.clone(),
        fingerprint:     handshake.session_fingerprint.clone(),
        streams_opened:  0,
        bytes_forwarded: 0,
        ended_at:        chrono::Utc::now(),
    }).await?;

    // Connect via Iroh — direct or relay, automatic, no STUN needed
    run_connector(&handshake, local).await?;

    Ok(())
}

/// Find a free local port for auto-assignment.
async fn find_free_port() -> anyhow::Result<u16> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    Ok(listener.local_addr()?.port())
}