//! Shell session configuration for Device B (the host).
//! Stored at ~/.punch/shell_config.json
//! Sensible defaults protect users who never configure it.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellConfig {
    /// Commands that are always blocked — never reach the PTY
    pub blocked_commands: Vec<String>,

    /// Paths that trigger auto-block when accessed
    pub blocked_paths: Vec<String>,

    /// Patterns that trigger a real-time alert on Device B
    /// Device B sees alert and can allow/block/always-block
    pub suspicious_patterns: Vec<String>,

    /// If true, shell stays alive when Device A disconnects
    /// Device A can reconnect and resume the session
    pub persist_on_disconnect: bool,

    /// Max session duration in minutes (0 = unlimited)
    pub max_session_minutes: u64,

    /// Shell to use (defaults to system shell)
    pub shell: Option<String>,
}

impl Default for ShellConfig {
    fn default() -> Self {
        ShellConfig {
            blocked_commands: vec![
                "rm -rf /".into(),
                "rm -rf /*".into(),
                "mkfs".into(),
                "dd if=/dev/zero".into(),
                "dd if=/dev/random".into(),
                ":(){:|:&};:".into(),       // fork bomb
                "chmod -R 777 /".into(),
                "chown -R".into(),
                "> /dev/sda".into(),
                "format c:".into(),         // Windows
            ],
            blocked_paths: vec![
                "/etc/shadow".into(),
                "/etc/sudoers".into(),
                "/boot".into(),
                "/proc/sysrq-trigger".into(),
            ],
            suspicious_patterns: vec![
                "/etc/passwd".into(),
                "~/.ssh".into(),
                ".ssh/id_rsa".into(),
                ".ssh/authorized_keys".into(),
                "sudo".into(),
                "su ".into(),
                "chmod 777".into(),
                "curl | bash".into(),
                "wget | bash".into(),
                "curl | sh".into(),
                "wget | sh".into(),
                "base64 -d".into(),
                "eval".into(),
                "history -c".into(),
                "/etc/crontab".into(),
                "nc -l".into(),             // reverse shell
                "ncat -l".into(),
                "bash -i".into(),
            ],
            persist_on_disconnect: false,
            max_session_minutes: 0,
            shell: None,
        }
    }
}

impl ShellConfig {
    pub fn config_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".punch").join("shell_config.json")
    }

    /// Load config from disk, or create default if not present.
    pub async fn load() -> Self {
        let path = Self::config_path();

        if path.exists() {
            match tokio::fs::read_to_string(&path).await {
                Ok(content) => {
                    match serde_json::from_str::<ShellConfig>(&content) {
                        Ok(cfg) => return cfg,
                        Err(e) => {
                            tracing::warn!("Shell config parse error: {} — using defaults", e);
                        }
                    }
                }
                Err(e) => tracing::warn!("Shell config read error: {} — using defaults", e),
            }
        }

        // First run — write defaults to disk
        let default = ShellConfig::default();
        let _ = default.save().await;
        default
    }

    pub async fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let json = serde_json::to_string_pretty(self)?;
        tokio::fs::write(&path, json).await?;
        Ok(())
    }

    /// Check if a command string matches any blocked pattern.
    /// Returns the matched pattern if blocked.
    pub fn is_blocked(&self, cmd: &str) -> Option<&str> {
        let cmd_lower = cmd.to_lowercase();
        for pattern in &self.blocked_commands {
            if cmd_lower.contains(&pattern.to_lowercase()) {
                return Some(pattern);
            }
        }
        for path in &self.blocked_paths {
            if cmd_lower.contains(&path.to_lowercase()) {
                return Some(path);
            }
        }
        None
    }

    /// Check if a command matches any suspicious pattern.
    /// Returns all matched patterns.
    pub fn suspicious_matches(&self, cmd: &str) -> Vec<&str> {
        let cmd_lower = cmd.to_lowercase();
        self.suspicious_patterns
            .iter()
            .filter(|p| cmd_lower.contains(&p.to_lowercase()))
            .map(|p| p.as_str())
            .collect()
    }

    /// Get the shell binary to use.
    pub fn shell_binary(&self) -> String {
        if let Some(s) = &self.shell {
            return s.clone();
        }
        #[cfg(windows)]
        return "cmd.exe".to_string();
        #[cfg(not(windows))]
        {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
        }
    }
}

/// Audit log entry for a shell session.
#[derive(Debug, Serialize, Deserialize)]
pub struct ShellSessionLog {
    pub session_id: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
    pub peer_node_id: String,
    pub token_type: String,
    pub commands: Vec<CommandEntry>,
    pub terminated_by: TerminatedBy,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub command: String,
    pub disposition: CommandDisposition,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CommandDisposition {
    Allowed,
    Blocked,
    SuspiciousAllowed,
    SuspiciousBlocked,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminatedBy {
    HostKilled,
    PeerDisconnected,
    SessionTimeout,
    PeerClosed,
}

pub fn shell_log_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".punch").join("logs").join("shell_sessions.json")
}

pub async fn write_shell_log(entry: ShellSessionLog) -> anyhow::Result<()> {
    let path = shell_log_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut entries: Vec<ShellSessionLog> = if path.exists() {
        let content = tokio::fs::read_to_string(&path).await?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        vec![]
    };
    entries.push(entry);
    tokio::fs::write(&path, serde_json::to_string_pretty(&entries)?).await?;
    Ok(())
}
