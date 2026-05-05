use crate::token::{Token, TokenType};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

/// A persisted token entry in ~/.punch/tokens.json
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredToken {
    pub code: String,
    pub token_type: StoredTokenType,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
#[allow(clippy::enum_variant_names)]
pub enum StoredTokenType {
    TNo,
    QNo { remaining: u32 },
    PNo { verified: bool },
}

impl StoredToken {
    /// Returns true if this token can still be used.
    pub fn is_valid(&self) -> bool {
        match &self.token_type {
            StoredTokenType::TNo => true,       // checked once then removed
            StoredTokenType::QNo { remaining } => *remaining > 0,
            StoredTokenType::PNo { verified } => *verified,
        }
    }

    /// Human readable status for display.
    pub fn status(&self) -> String {
        match &self.token_type {
            StoredTokenType::TNo => "Temporary (single use)".to_string(),
            StoredTokenType::QNo { remaining } => format!("Quantised ({} uses remaining)", remaining),
            StoredTokenType::PNo { verified } => {
                if *verified { "Permanent (verified)".to_string() }
                else { "Permanent (not verified)".to_string() }
            }
        }
    }
}

fn token_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".punch").join("tokens.json")
}

/// Load all stored tokens from disk.
pub async fn load_tokens() -> Vec<StoredToken> {
    let path = token_path();
    if !path.exists() { return vec![]; }
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => vec![],
    }
}

/// Save all tokens back to disk.
async fn save_tokens(tokens: &[StoredToken]) -> anyhow::Result<()> {
    let path = token_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut file = tokio::fs::File::create(&path).await?;
    let json = serde_json::to_string_pretty(tokens)?;
    file.write_all(json.as_bytes()).await?;
    Ok(())
}

/// Persist a newly generated token.
/// T-No tokens are NOT stored — they are ephemeral by design.
pub async fn store_token(token: &Token) -> anyhow::Result<()> {
    // T-No is ephemeral — nothing to store
    if token.token_type == TokenType::TNo {
        return Ok(());
    }

    let stored = StoredToken {
        code: token.code.clone(),
        token_type: match &token.token_type {
            TokenType::TNo => StoredTokenType::TNo,
            TokenType::QNo { remaining } => StoredTokenType::QNo { remaining: *remaining },
            TokenType::PNo => StoredTokenType::PNo { verified: false },
        },
        created_at: Utc::now(),
        last_used: None,
    };

    let mut tokens = load_tokens().await;
    tokens.push(stored);
    save_tokens(&tokens).await
}

/// Check if a token is valid and enforce its policy.
/// Returns Ok(()) if connection should proceed.
/// Returns Err with a human readable reason if it should be rejected.
pub async fn check_and_consume(code: &str) -> anyhow::Result<()> {
    let mut tokens = load_tokens().await;

    // Find the token
    let pos = tokens.iter().position(|t| t.code == code);

    match pos {
        None => Ok(()),
        Some(i) => {
            let token = &tokens[i];

            if !token.is_valid() {
                anyhow::bail!(
                    "Token {} has expired and can no longer be used.",
                    code
                );
            }

            // Update state based on type
            match tokens[i].token_type.clone() {
                StoredTokenType::TNo => {
                    // Remove after single use
                    tokens.remove(i);
                }
                StoredTokenType::QNo { remaining } => {
                    if remaining == 1 {
                        // Last use — remove it
                        println!("⚠️  Last use of Q-No token {}.", code);
                        tokens.remove(i);
                    } else {
                        tokens[i].token_type = StoredTokenType::QNo {
                            remaining: remaining - 1,
                        };
                        tokens[i].last_used = Some(Utc::now());
                        println!("Q-No token: {} uses remaining.", remaining - 1);
                    }
                }
                StoredTokenType::PNo { verified } => {
                    if !verified {
                        anyhow::bail!(
                            "P-No token {} requires verification before use. \
                             Run: punch verify {}",
                            code, code
                        );
                    }
                    tokens[i].last_used = Some(Utc::now());
                }
            }

            save_tokens(&tokens).await
        }
    }
}

/// Verify a P-No token after explicit user confirmation.
pub async fn verify_pno_token(code: &str) -> anyhow::Result<()> {
    let mut tokens = load_tokens().await;

    let token = tokens.iter_mut().find(|t| t.code == code)
        .ok_or_else(|| anyhow::anyhow!("Token {} not found.", code))?;

    match &token.token_type {
        StoredTokenType::PNo { .. } => {
            token.token_type = StoredTokenType::PNo { verified: true };
            save_tokens(&tokens).await?;
            println!("✅ Token {} verified. Permanent access enabled.", code);
            Ok(())
        }
        _ => anyhow::bail!("Token {} is not a P-No token.", code),
    }
}

/// List all stored tokens — used by dashboard and CLI.
pub async fn list_tokens() -> Vec<StoredToken> {
    load_tokens().await
}

/// Revoke a token manually.
pub async fn revoke_token(code: &str) -> anyhow::Result<()> {
    let mut tokens = load_tokens().await;
    let before = tokens.len();
    tokens.retain(|t| t.code != code);

    if tokens.len() == before {
        anyhow::bail!("Token {} not found.", code);
    }

    save_tokens(&tokens).await?;
    println!("🗑️  Token {} revoked.", code);
    Ok(())
}
