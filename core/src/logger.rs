use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionType {
    Direct,
    Relay,
}

/// A single session log entry.
/// Stored locally on the device only — never sent anywhere.
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionLog {
    pub session_id: String,
    pub token_code: String,
    pub connection_type: ConnectionType,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub ended_at: chrono::DateTime<chrono::Utc>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

impl SessionLog {
    #[allow(dead_code)]
    pub fn duration_seconds(&self) -> i64 {
        (self.ended_at - self.started_at).num_seconds()
    }
}

/// Returns the path to the local log file.
pub fn log_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".punch").join("logs").join("sessions.json")
}

/// Appends a session log entry to the local log file.
/// The log file is a JSON array of session entries.
pub async fn write_log(entry: SessionLog) -> anyhow::Result<()> {
    let path = log_path();

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Read existing logs
    let mut logs: Vec<SessionLog> = if path.exists() {
        let content = tokio::fs::read_to_string(&path).await?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        vec![]
    };

    logs.push(entry);

    // Write back
    let mut file = tokio::fs::File::create(&path).await?;
    let json = serde_json::to_string_pretty(&logs)?;
    file.write_all(json.as_bytes()).await?;

    Ok(())
}
