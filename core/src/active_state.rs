//! Active session state — written by Punch modules when sessions start/stop.
//! Dashboard reads this via /api/active to show live connections.
//! File: ~/.punch/logs/active.json

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActiveState {
    pub forwards: Vec<ActiveForward>,
    pub shells:   Vec<ActiveShell>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveForward {
    pub id:           String,
    pub port:         u16,
    pub protocol:     String,
    pub token_type:   String,
    pub fingerprint:  String,
    pub started_at:   String,
    pub stream_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveShell {
    pub id:           String,
    pub peer_node_id: String,
    pub token_type:   String,
    pub fingerprint:  String,
    pub started_at:   String,
    pub cmd_count:    u32,
}

fn active_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".punch").join("logs").join("active.json")
}

async fn load() -> ActiveState {
    let path = active_path();
    if !path.exists() { return ActiveState::default(); }
    match tokio::fs::read_to_string(&path).await {
        Ok(c) => serde_json::from_str(&c).unwrap_or_default(),
        Err(_) => ActiveState::default(),
    }
}

async fn save(state: &ActiveState) {
    let path = active_path();
    if let Some(p) = path.parent() {
        let _ = tokio::fs::create_dir_all(p).await;
    }
    if let Ok(json) = serde_json::to_string_pretty(state) {
        if let Ok(mut f) = tokio::fs::File::create(&path).await {
            let _ = f.write_all(json.as_bytes()).await;
        }
    }
}

// ─── FORWARD ─────────────────────────────────────────────────────────────────

pub async fn register_forward(entry: ActiveForward) {
    let mut state = load().await;
    state.forwards.retain(|f| f.id != entry.id);
    state.forwards.push(entry.clone());
    save(&state).await;

    crate::dashboard_server::emit("forward_start", serde_json::json!({
        "id":          entry.id,
        "port":        entry.port,
        "protocol":    entry.protocol,
        "token_type":  entry.token_type,
        "fingerprint": entry.fingerprint,
        "started_at":  entry.started_at,
    }));
}

pub async fn update_forward_streams(id: &str, count: u32) {
    let mut state = load().await;
    if let Some(f) = state.forwards.iter_mut().find(|f| f.id == id) {
        f.stream_count = count;
    }
    save(&state).await;

    crate::dashboard_server::emit("forward_streams", serde_json::json!({
        "id": id, "stream_count": count
    }));
}

pub async fn deregister_forward(id: &str) {
    let mut state = load().await;
    state.forwards.retain(|f| f.id != id);
    save(&state).await;

    crate::dashboard_server::emit("forward_end", serde_json::json!({ "id": id }));
}

// ─── SHELL ───────────────────────────────────────────────────────────────────

pub async fn register_shell(entry: ActiveShell) {
    let mut state = load().await;
    state.shells.retain(|s| s.id != entry.id);
    state.shells.push(entry.clone());
    save(&state).await;

    crate::dashboard_server::emit("shell_start", serde_json::json!({
        "id":           entry.id,
        "peer_node_id": entry.peer_node_id,
        "token_type":   entry.token_type,
        "fingerprint":  entry.fingerprint,
        "started_at":   entry.started_at,
    }));
}

pub async fn shell_command_event(session_id: &str, command: &str, disposition: &str) {
    // Increment cmd_count in active state
    let mut state = load().await;
    if let Some(s) = state.shells.iter_mut().find(|s| s.id == session_id) {
        s.cmd_count += 1;
    }
    save(&state).await;

    crate::dashboard_server::emit("shell_command", serde_json::json!({
        "session_id":  session_id,
        "command":     command,
        "disposition": disposition,
        "timestamp":   chrono::Utc::now().to_rfc3339(),
    }));
}

pub async fn deregister_shell(id: &str) {
    let mut state = load().await;
    state.shells.retain(|s| s.id != id);
    save(&state).await;

    crate::dashboard_server::emit("shell_end", serde_json::json!({ "id": id }));
}
