//! File transfer safety — risk classification, consent protocol,
//! and mandatory local acceptance logging.
//!
//! Philosophy: inform, never block. User always decides.
//! Acceptance is always logged locally regardless of --log flag.

#![allow(dead_code)]

use std::path::Path;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

/// Risk level of a file based on its extension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RiskLevel {
    High,   // executables, scripts
    Medium, // archives (may contain executables)
    Low,    // documents, media
}

impl RiskLevel {
    pub fn icon(&self) -> &'static str {
        match self {
            RiskLevel::High   => "🔴",
            RiskLevel::Medium => "🟡",
            RiskLevel::Low    => "🟢",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            RiskLevel::High   => "HIGH RISK",
            RiskLevel::Medium => "MEDIUM RISK",
            RiskLevel::Low    => "LOW RISK",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            RiskLevel::High   => "Executable or script — can run code on your system",
            RiskLevel::Medium => "Archive — may contain executable files inside",
            RiskLevel::Low    => "Document or media — generally safe",
        }
    }
}

const HIGH_RISK_EXTENSIONS: &[&str] = &[
    "exe", "bat", "cmd", "com", "msi", "dll", "so", "dylib",
    "sh", "bash", "zsh", "fish", "ps1", "psm1", "psd1",
    "app", "dmg", "pkg", "deb", "rpm",
    "vbs", "vbe", "js", "jse", "wsf", "wsh",
    "scr", "pif", "reg", "inf",
];

const MEDIUM_RISK_EXTENSIONS: &[&str] = &[
    "zip", "tar", "gz", "tgz", "bz2", "xz", "7z", "rar",
    "jar", "war", "ear", "apk", "ipa",
];

/// Classify the risk level of a file by its extension.
pub fn classify(filename: &str) -> RiskLevel {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if HIGH_RISK_EXTENSIONS.contains(&ext.as_str()) {
        RiskLevel::High
    } else if MEDIUM_RISK_EXTENSIONS.contains(&ext.as_str()) {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    }
}

/// Generate a short session fingerprint for verbal verification.
/// Both sides see the same fingerprint — derived from file checksum.
pub fn session_fingerprint(checksum: &str) -> String {
    // Take first 12 chars of checksum, group into 3x4 for readability
    let chars: String = checksum.chars().take(12).collect();
    format!("{}-{}-{}", &chars[0..4], &chars[4..8], &chars[8..12])
}

/// Information shown to the receiver before they accept.
pub struct TransferConsent {
    pub filename: String,
    pub size_mb: f64,
    pub extension: String,
    pub risk: RiskLevel,
    pub checksum: String,
    pub fingerprint: String,
    pub sender_addr: String,
}

impl TransferConsent {
    pub fn build(
        filename: &str,
        total_size: u64,
        checksum: &str,
        sender_addr: &str,
    ) -> Self {
        let ext = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown")
            .to_string();

        TransferConsent {
            filename: filename.to_string(),
            size_mb: total_size as f64 / (1024.0 * 1024.0),
            extension: ext,
            risk: classify(filename),
            checksum: checksum.to_string(),
            fingerprint: session_fingerprint(checksum),
            sender_addr: sender_addr.to_string(),
        }
    }

    /// Display the consent prompt and return true if user accepts.
    pub async fn prompt(&self) -> anyhow::Result<bool> {
        println!("\n{}", "─".repeat(50));
        println!("  📦 Incoming file transfer request");
        println!("{}", "─".repeat(50));
        println!("  File:        {}", self.filename);
        println!("  Size:        {:.1} MB", self.size_mb);
        println!("  Type:        .{}", self.extension);
        println!("  Risk:        {} {}", self.risk.icon(), self.risk.label());
        println!("  Info:        {}", self.risk.description());
        println!("  Checksum:    {}...", &self.checksum[..16]);
        println!("  Fingerprint: {} (verify with sender)", self.fingerprint);
        println!("{}", "─".repeat(50));

        // Extra warning for high risk
        if self.risk == RiskLevel::High {
            println!();
            println!("  ⚠️  WARNING: This file type can execute code on your system.");
            println!("  Only accept from people you fully trust.");
            println!("  Punch cannot verify the contents of this file.");
        }

        if self.risk == RiskLevel::Medium {
            println!();
            println!("  ⚠️  Note: Archives may contain executable files.");
            println!("  Scan with antivirus after receiving.");
        }

        println!();
        println!("  Tip: Ask the sender to confirm fingerprint: {}", self.fingerprint);
        println!();
        print!("  Accept? (yes/no): ");

        // Flush stdout so prompt appears
        use std::io::Write;
        std::io::stdout().flush()?;

        // 30 second timeout on acceptance
        let decision = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            read_line_async()
        ).await;

        match decision {
            Ok(Ok(input)) => {
                let accepted = input.trim().to_lowercase() == "yes";
                Ok(accepted)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => {
                println!("\n  Timed out — transfer rejected.");
                Ok(false)
            }
        }
    }
}

async fn read_line_async() -> anyhow::Result<String> {
    let mut input = String::new();
    // Run blocking stdin read on a separate thread
    tokio::task::spawn_blocking(move || {
        std::io::stdin().read_line(&mut input)?;
        Ok::<String, std::io::Error>(input)
    })
    .await?
    .map_err(|e| anyhow::anyhow!("Input error: {}", e))
}

/// Sender-side display before sending.
pub fn display_sender_info(filename: &str, total_size: u64, checksum: &str) {
    let risk        = classify(filename);
    let fingerprint = session_fingerprint(checksum);
    let size_mb     = total_size as f64 / (1024.0 * 1024.0);

    println!("\n{}", "─".repeat(50));
    println!("  📤 Sending file");
    println!("{}", "─".repeat(50));
    println!("  File:        {}", filename);
    println!("  Size:        {:.1} MB", size_mb);
    println!("  Risk level:  {} {}", risk.icon(), risk.label());
    println!("  Fingerprint: {}", fingerprint);
    println!("{}", "─".repeat(50));
    println!("  Share fingerprint with receiver for verification.");

    if risk == RiskLevel::High {
        println!();
        println!("  ⚠️  You are sending a HIGH RISK file type.");
        println!("  The receiver will see a strong warning before accepting.");
    }
    println!();
}

// ─── ACCEPTANCE LOG ───────────────────────────────────────────────────────────

/// A record of every transfer acceptance decision — always logged locally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceRecord {
    pub timestamp: chrono::DateTime<Utc>,
    pub filename: String,
    pub size_bytes: u64,
    pub checksum: String,
    pub fingerprint: String,
    pub risk_level: String,
    pub sender_addr: String,
    pub decision: String, // "accepted" | "rejected" | "timeout"
    pub dest_path: String,
}

fn acceptance_log_path() -> std::path::PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    home.join(".punch").join("logs").join("transfers.json")
}

/// Log a transfer acceptance decision locally.
/// Always called regardless of --log flag — this is a safety record.
pub async fn log_acceptance(
    consent: &TransferConsent,
    total_size: u64,
    accepted: bool,
    dest_path: &str,
) {
    let record = AcceptanceRecord {
        timestamp:   Utc::now(),
        filename:    consent.filename.clone(),
        size_bytes:  total_size,
        checksum:    consent.checksum.clone(),
        fingerprint: consent.fingerprint.clone(),
        risk_level:  consent.risk.label().to_string(),
        sender_addr: consent.sender_addr.clone(),
        decision:    if accepted { "accepted" } else { "rejected" }.to_string(),
        dest_path:   dest_path.to_string(),
    };

    // Best-effort — never fail the transfer over a log write error
    if let Err(e) = write_acceptance_record(record).await {
        tracing::warn!("Could not write acceptance log: {}", e);
    }
}

async fn write_acceptance_record(record: AcceptanceRecord) -> anyhow::Result<()> {
    let path = acceptance_log_path();

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut records: Vec<AcceptanceRecord> = if path.exists() {
        let content = tokio::fs::read_to_string(&path).await?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        vec![]
    };

    records.push(record.clone());

    crate::dashboard_server::emit(
        "transfer_decision",
        serde_json::json!({
            "filename": record.filename,
            "size_bytes": record.size_bytes,
            "decision": record.decision,
            "risk_level": record.risk_level,
            "fingerprint": record.fingerprint,
            "dest_path": record.dest_path,
        }),
    );

    let mut file = tokio::fs::File::create(&path).await?;
    file.write_all(serde_json::to_string_pretty(&records)?.as_bytes()).await?;
    Ok(())
}
