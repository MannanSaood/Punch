use rand::Rng;
use serde::{Deserialize, Serialize};

/// The type of access token being generated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TokenType {
    /// Temporary — single session, expires after use.
    TNo,
    /// Quantised — expires after N connections.
    QNo { remaining: u32 },
    /// Permanent — persistent, requires extra verification.
    PNo,
}

impl std::fmt::Display for TokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenType::TNo => write!(f, "T-No"),
            TokenType::QNo { remaining } => write!(f, "Q-No ({}x remaining)", remaining),
            TokenType::PNo => write!(f, "P-No"),
        }
    }
}

/// A generated connection token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub code: String,
    pub token_type: TokenType,
}

impl Token {
    /// Generate a new token based on the requested type.
    pub fn generate(uses: Option<u32>, permanent: bool) -> Self {
        let code = Self::random_code();
        let token_type = if permanent {
            TokenType::PNo
        } else if let Some(n) = uses {
            TokenType::QNo { remaining: n }
        } else {
            TokenType::TNo
        };

        Token { code, token_type }
    }

    /// Generate a random 4-6 digit code.
    fn random_code() -> String {
        let mut rng = rand::thread_rng();
        // 4 digits — simple and readable
        format!("{:04}", rng.gen_range(1000..9999))
    }

    /// Returns true if this token allows another connection.
    pub fn can_connect(&self) -> bool {
        match &self.token_type {
            TokenType::TNo => true, // checked once, then expired
            TokenType::QNo { remaining } => *remaining > 0,
            TokenType::PNo => true,
        }
    }

    /// Returns the display label for the code.
    pub fn display_label(&self) -> &str {
        match &self.token_type {
            TokenType::TNo => "T-No",
            TokenType::QNo { .. } => "Q-No",
            TokenType::PNo => "P-No",
        }
    }
}
