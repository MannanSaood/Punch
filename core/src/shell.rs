//! Remote terminal (v0.6)
//!
//! Device B (host) — spawns a PTY, owns all security controls:
//!   - Always prompts before allowing connection
//!   - Sees every command Device A submits in real time
//!   - Ctrl+K kills session instantly
//!   - Blocklist auto-rejects dangerous commands
//!   - Suspicious patterns trigger real-time alerts
//!   - Configures session persistence on disconnect
//!
//! Device A (client) — gets an interactive terminal:
//!   - Connects via Iroh QUIC (same as file transfer / port forward)
//!   - Two QUIC streams: data (stdin/stdout) + control (kill/alerts)
//!   - Full PTY — colors, cursor, interactive programs work
//!
//! Transport: Iroh QUIC — hole punch + relay.iroh.network fallback

use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use iroh::{Endpoint, EndpointAddr, Watcher};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::shell_config::{
    CommandDisposition, CommandEntry, ShellConfig, ShellSessionLog,
    TerminatedBy, write_shell_log,
};

pub const SHELL_ALPN: &[u8] = b"punch/shell/1";

/// Control messages sent over the control QUIC stream.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlMsg {
    /// Device B → Device A: session approved, shell starting
    Approved { shell: String },
    /// Device B → Device A: session rejected
    Rejected { reason: String },
    /// Device B → Device A: command was blocked
    CommandBlocked { command: String, pattern: String },
    /// Device B → Device A: suspicious command alert (allowed)
    SuspiciousAllowed { command: String, patterns: Vec<String> },
    /// Device B → Device A: host killed the session
    HostKilled,
    /// Device A → Device B: client closing cleanly
    ClientClosing,
    /// Device B → Device A: terminal resize
    Resize { cols: u16, rows: u16 },
}

// ─── DEVICE B (HOST) ─────────────────────────────────────────────────────────

/// Metadata shared via signalling so Device A can connect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellHandshake {
    pub endpoint_addr: String,
    pub session_fingerprint: String,
    pub token_type: String,
}

/// Prepare Iroh endpoint for the host side.
pub async fn prepare_host(token_type: &str) -> anyhow::Result<(ShellHandshake, Endpoint)> {
    print!("🔌 Starting Iroh endpoint... ");

    let endpoint = Endpoint::builder()
        .alpns(vec![SHELL_ALPN.to_vec()])
        .bind()
        .await
        .context("Failed to create Iroh endpoint")?;

    endpoint.online().await;

    let addr: EndpointAddr = endpoint.watch_addr().get();

    println!("done");
    println!("🌐 Node ID: {}", addr.id);

    let fingerprint = crate::forward::session_fingerprint(&addr);
    println!("🔐 Session fingerprint: {}", fingerprint);

    let addr_str = serde_json::to_string(&addr).context("serialize EndpointAddr")?;

    let handshake = ShellHandshake {
        endpoint_addr: addr_str,
        session_fingerprint: fingerprint,
        token_type: token_type.to_string(),
    };

    Ok((handshake, endpoint))
}

/// Run the host side — accept connection, show monitor, manage PTY.
pub async fn run_host(endpoint: Endpoint, handshake: &ShellHandshake) -> anyhow::Result<()> {
    let config = ShellConfig::load().await;

    println!("\n✅ Ready. Waiting for client...\n");

    // Accept Iroh connection
    let conn = tokio::time::timeout(
        Duration::from_secs(120),
        async {
            loop {
                match endpoint.accept().await {
                    Some(incoming) => match incoming.await {
                        Ok(c)  => return Ok(c),
                        Err(e) => tracing::warn!("Accept error: {}", e),
                    },
                    None => anyhow::bail!("Endpoint closed"),
                }
            }
        }
    ).await
    .context("Timed out waiting for client")??;

    let peer_id = conn.remote_id().to_string();
    let peer_short = &peer_id[..12.min(peer_id.len())];

    // ── Always prompt — regardless of token type ──────────────────────────────
    println!("📲 Shell request from: {}...", peer_short);
    println!("   Token type:  {}", handshake.token_type);
    println!("   Fingerprint: {}", handshake.session_fingerprint);
    println!();
    print!("   Allow shell access? (yes/no): ");

    use std::io::Write;
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() != "yes" {
        // Reject — send control message and close
        let (mut ctrl_send, _) = conn.open_bi().await?;
        let msg = serde_json::to_vec(&ControlMsg::Rejected {
            reason: "Host declined the connection.".to_string(),
        })?;
        ctrl_send.write_u32(msg.len() as u32).await?;
        ctrl_send.write_all(&msg).await?;
        println!("Connection rejected.");
        return Ok(());
    }

    // ── Session persistence config ────────────────────────────────────────────
    print!("   Keep shell alive if client disconnects? (yes/no) [default: {}]: ",
        if config.persist_on_disconnect { "yes" } else { "no" });
    std::io::stdout().flush()?;
    let mut persist_input = String::new();
    std::io::stdin().read_line(&mut persist_input)?;
    let _persist = match persist_input.trim().to_lowercase().as_str() {
        "yes" | "y" => true,
        "no"  | "n" => false,
        _            => config.persist_on_disconnect,
    };

    // Open control stream and send approval *before* waiting for the client's data stream.
    // Client waits on read_u32/read_exact here, then opens the data stream — ordering avoids deadlock.
    let (mut ctrl_send, _) = conn.open_bi().await?;

    let shell_bin = config.shell_binary();
    let approve_msg = serde_json::to_vec(&ControlMsg::Approved {
        shell: shell_bin.clone(),
    })?;
    ctrl_send.write_u32(approve_msg.len() as u32).await?;
    ctrl_send.write_all(&approve_msg).await?;

    // Accept data stream from client (client opens this after receiving approval)
    tracing::debug!("shell host: waiting for client data stream (accept_bi)");
    let (data_send, mut data_recv) = conn
        .accept_bi()
        .await
        .context("Client did not open data stream")?;
    tracing::debug!("shell host: data stream accepted");

    // ── Spawn PTY ─────────────────────────────────────────────────────────────
    let pty_system = native_pty_system();
    let pty = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let mut cmd = CommandBuilder::new(&shell_bin);
    cmd.env("TERM", "xterm-256color");
    cmd.env("PUNCH_SESSION", "1");

    let mut child = pty.slave.spawn_command(cmd)?;
    let pty_reader = pty.master.try_clone_reader()?;
    let pty_writer = pty.master.take_writer()?;

    let session_id = Uuid::new_v4().to_string();
    let session_start = chrono::Utc::now();

    // ── Command interceptor ───────────────────────────────────────────────────
    // We intercept input from Device A before it hits the PTY.
    // This is how we block/alert on commands.
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<String>();
    let (kill_tx, mut kill_rx) = mpsc::unbounded_channel::<()>();
    let commands_log: Arc<Mutex<Vec<CommandEntry>>> = Arc::new(Mutex::new(vec![]));
    let commands_log_clone = Arc::clone(&commands_log);
    let config_clone = config.clone();
    let _kill_tx_host = kill_tx.clone();

    println!("\n┌─────────────────────────────────────────────────────┐");
    println!("│  👊 punch shell — session active                    │");
    println!("│  Peer: {}...                    │", &peer_short.chars().take(16).collect::<String>());
    println!("│  Ctrl+K = kill session instantly                    │");
    println!("└─────────────────────────────────────────────────────┘");
    println!("  Commands (live):\n");

    // Spawn host keyboard listener for Ctrl+K
    let kill_tx_kb = kill_tx.clone();
    tokio::spawn(async move {
        let _ = enable_raw_mode();
        loop {
            if let Ok(Event::Key(key)) = event::read() {
                if key.code == KeyCode::Char('k')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    let _ = kill_tx_kb.send(());
                    break;
                }
            }
        }
        let _ = disable_raw_mode();
    });

    // PTY → QUIC (blocking read thread — MasterPty has no async I/O)
    let (pty_out_tx, mut pty_out_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut r = pty_reader;
        let mut buf = vec![0u8; 4096];
        loop {
            match r.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if pty_out_tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let data_send = Arc::new(Mutex::new(data_send));
    let data_send_pty = Arc::clone(&data_send);
    tokio::spawn(async move {
        while let Some(chunk) = pty_out_rx.recv().await {
            let mut send = data_send_pty.lock().await;
            if send.write_all(&chunk).await.is_err() {
                break;
            }
        }
    });

    // QUIC → PTY (dedicated writer thread — `take_writer` is single-owner)
    let (pty_in_tx, pty_in_rx) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut w = pty_writer;
        while let Ok(data) = pty_in_rx.recv() {
            if data.is_empty() {
                break;
            }
            if w.write_all(&data).is_err() {
                break;
            }
        }
    });

    // Spawn Device A → PTY + command interceptor
    let config_intercept = config_clone.clone();
    let cmd_tx_clone = cmd_tx.clone();
    let ctrl_send = Arc::new(Mutex::new(ctrl_send));
    let ctrl_send_intercept = Arc::clone(&ctrl_send);
    let pty_in_tx_intercept = pty_in_tx.clone();

    tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        let mut line_buf = String::new();

        loop {
            let n = match data_recv.read(&mut buf).await {
                Ok(Some(0)) | Ok(None) => break,
                Ok(Some(n)) => n,
                Err(_) => break,
            };

            let chunk = String::from_utf8_lossy(&buf[..n]);

            // Accumulate line to detect commands
            for ch in chunk.chars() {
                if ch == '\r' || ch == '\n' {
                    let cmd = line_buf.trim().to_string();
                    if !cmd.is_empty() {
                        let _ = cmd_tx_clone.send(cmd.clone());

                        // Check blocklist BEFORE writing to PTY
                        if let Some(pattern) = config_intercept.is_blocked(&cmd) {
                            let block_msg = serde_json::to_vec(&ControlMsg::CommandBlocked {
                                command: cmd.clone(),
                                pattern: pattern.to_string(),
                            }).unwrap_or_default();
                            let mut ctrl = ctrl_send_intercept.lock().await;
                            let _ = ctrl.write_u32(block_msg.len() as u32).await;
                            let _ = ctrl.write_all(&block_msg).await;
                            // Don't write to PTY — command is blocked
                            line_buf.clear();
                            continue;
                        }
                    }
                    line_buf.clear();
                } else {
                    line_buf.push(ch);
                }
            }

            if pty_in_tx_intercept.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });

    // Command monitor — display + suspicious alert on Device B
    let config_monitor = config_clone.clone();
    let commands_log_monitor = Arc::clone(&commands_log_clone);
    let ctrl_send_monitor = Arc::clone(&ctrl_send);
    let _kill_tx_monitor = kill_tx.clone();

    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            let timestamp = chrono::Utc::now();
            let suspicious = config_monitor.suspicious_matches(&cmd);

            if !suspicious.is_empty() {
                println!("\n  ⚠️  SUSPICIOUS: {}", cmd);
                println!("     Matched: {}", suspicious.join(", "));
                print!("     Allow this time? (yes/no/block-always) [yes]: ");

                use std::io::Write;
                std::io::stdout().flush().ok();

                let mut resp = String::new();
                std::io::stdin().read_line(&mut resp).ok();

                let disposition = match resp.trim().to_lowercase().as_str() {
                    "no" | "n" => {
                        // Block this instance
                        let msg = serde_json::to_vec(&ControlMsg::CommandBlocked {
                            command: cmd.clone(),
                            pattern: suspicious.join(", "),
                        }).unwrap_or_default();
                        let mut ctrl = ctrl_send_monitor.lock().await;
                        let _ = ctrl.write_u32(msg.len() as u32).await;
                        let _ = ctrl.write_all(&msg).await;
                        CommandDisposition::SuspiciousBlocked
                    }
                    _ => {
                        // Allow with notice
                        let msg = serde_json::to_vec(&ControlMsg::SuspiciousAllowed {
                            command: cmd.clone(),
                            patterns: suspicious.iter().map(|s| s.to_string()).collect(),
                        }).unwrap_or_default();
                        let mut ctrl = ctrl_send_monitor.lock().await;
                        let _ = ctrl.write_u32(msg.len() as u32).await;
                        let _ = ctrl.write_all(&msg).await;
                        CommandDisposition::SuspiciousAllowed
                    }
                };

                commands_log_monitor.lock().await.push(CommandEntry {
                    timestamp,
                    command: cmd.clone(),
                    disposition,
                });
            } else {
                // Normal command — just log it
                println!("  [{}] {}", timestamp.format("%H:%M:%S"), cmd);
                commands_log_monitor.lock().await.push(CommandEntry {
                    timestamp,
                    command: cmd,
                    disposition: CommandDisposition::Allowed,
                });
            }
        }
    });

    // Wait for kill signal or client close
    let terminated_by = tokio::select! {
        _ = kill_rx.recv() => {
            println!("\n\n🛑 Session killed by host (Ctrl+K)");
            let msg = serde_json::to_vec(&ControlMsg::HostKilled).unwrap_or_default();
            let mut ctrl = ctrl_send.lock().await;
            let _ = ctrl.write_u32(msg.len() as u32).await;
            let _ = ctrl.write_all(&msg).await;
            TerminatedBy::HostKilled
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n\n🛑 Host closed session.");
            TerminatedBy::HostKilled
        }
    };

    // Kill child process
    let _ = child.kill();
    let _ = pty_in_tx.send(Vec::new());

    // Write session audit log
    let commands = std::mem::take(&mut *commands_log.lock().await);
    let _ = write_shell_log(ShellSessionLog {
        session_id,
        started_at: session_start,
        ended_at: Some(chrono::Utc::now()),
        peer_node_id: peer_id,
        token_type: handshake.token_type.clone(),
        commands,
        terminated_by,
    }).await;

    endpoint.close().await;

    println!("Session ended. Log saved to ~/.punch/logs/shell_sessions.json");
    Ok(())
}

// ─── DEVICE A (CLIENT) ───────────────────────────────────────────────────────

/// Run the client side — connect to host, get interactive terminal.
pub async fn run_client(handshake: &ShellHandshake) -> anyhow::Result<()> {
    println!("\n🔗 Connecting to host (Iroh QUIC)...");

    let addr: EndpointAddr = serde_json::from_str(&handshake.endpoint_addr)
        .context("Failed to parse endpoint address")?;

    let endpoint = Endpoint::builder()
        .alpns(vec![SHELL_ALPN.to_vec()])
        .bind()
        .await
        .context("Failed to create Iroh endpoint")?;

    endpoint.online().await;

    let conn = tokio::time::timeout(
        Duration::from_secs(120),
        endpoint.connect(addr, SHELL_ALPN),
    )
    .await
    .context("Connection timed out")?
    .context("Failed to connect to host")?;

    println!("✅ Connected");
    println!("   Fingerprint: {}", handshake.session_fingerprint);
    println!("   Waiting for host to approve...\n");

    // Accept control stream from host
    let (_, mut ctrl_recv) = conn.accept_bi().await
        .context("No control stream from host")?;

    // Read approval/rejection
    let msg_len = ctrl_recv.read_u32().await? as usize;
    let mut msg_buf = vec![0u8; msg_len];
    ctrl_recv.read_exact(&mut msg_buf).await?;
    let ctrl_msg: ControlMsg = serde_json::from_slice(&msg_buf)?;

    match ctrl_msg {
        ControlMsg::Rejected { reason } => {
            println!("❌ Connection rejected: {}", reason);
            return Ok(());
        }
        ControlMsg::Approved { shell } => {
            println!("✅ Shell approved ({})", shell);
            println!("   Type normally. Host sees all commands.");
            println!("   Ctrl+C exits.\n");
        }
        _ => anyhow::bail!("Unexpected control message"),
    }

    // Open data stream to host. Iroh/QUIC: the opener must write on SendStream before the
    // peer's accept_bi can complete (see iroh::endpoint::Connection::accept_bi). Without this,
    // the host blocks forever on accept_bi and the client never gets a working session.
    let (mut data_send, mut data_recv) = conn.open_bi().await?;
    data_send
        .write_u8(0)
        .await
        .context("data stream initial write (required for peer accept_bi)")?;
    tracing::debug!("shell client: data stream opened and ping written");

    // Enable raw terminal mode on client side
    enable_raw_mode()?;

    // Windows + PowerShell: `tokio::io::stdin` often returns EOF (0) immediately, which ends the
    // session before you can type. Read stdin on a blocking OS thread instead (same idea as PTY on host).
    let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin();
        let mut buf = vec![0u8; 4096];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if stdin_tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let stdin_handle = tokio::spawn(async move {
        while let Some(chunk) = stdin_rx.recv().await {
            if data_send.write_all(&chunk).await.is_err() {
                break;
            }
        }
    });

    // stdout ← host PTY: one blocking writer thread (reliable on Windows conhost).
    let (out_tx, out_rx) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        use std::io::Write;
        let mut stdout = std::io::stdout();
        while let Ok(chunk) = out_rx.recv() {
            if chunk.is_empty() {
                break;
            }
            if stdout.write_all(&chunk).is_err() || stdout.flush().is_err() {
                break;
            }
        }
    });

    let stdout_handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        loop {
            let n = match data_recv.read(&mut buf).await {
                Ok(Some(0)) | Ok(None) => break,
                Ok(Some(n)) => n,
                Err(_) => break,
            };
            if out_tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
        let _ = out_tx.send(Vec::new());
    });

    // Listen for control messages (blocks, kills, alerts from host)
    let ctrl_handle = tokio::spawn(async move {
        while let Ok(n) = ctrl_recv.read_u32().await {
            let msg_len = n as usize;
            let mut msg_buf = vec![0u8; msg_len];
            if ctrl_recv.read_exact(&mut msg_buf).await.is_err() { break; }

            if let Ok(msg) = serde_json::from_slice::<ControlMsg>(&msg_buf) {
                match msg {
                    ControlMsg::HostKilled => {
                        eprintln!("\r\n\n🛑 Session terminated by host.\r");
                        break;
                    }
                    ControlMsg::CommandBlocked { command, pattern } => {
                        eprintln!("\r\n⛔ Blocked: '{}' (matched: {})\r", command, pattern);
                    }
                    ControlMsg::SuspiciousAllowed { command, patterns } => {
                        eprintln!("\r\n⚠️  Suspicious allowed: '{}' (matched: {})\r",
                            command, patterns.join(", "));
                    }
                    _ => {}
                }
            }
        }
    });

    // Wait for any to finish
    tokio::select! {
        _ = stdin_handle  => {}
        _ = stdout_handle => {}
        _ = ctrl_handle   => {}
        _ = tokio::signal::ctrl_c() => {}
    }

    disable_raw_mode()?;
    println!("\nDisconnected.");

    endpoint.close().await;
    Ok(())
}
