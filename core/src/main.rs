mod cli;
mod crypto;
mod stun;
mod signaling;
mod punch;
mod token;
mod logger;

use clap::{Parser, Subcommand};

const DEFAULT_SERVER: &str = "wss://punch.mannansaood.dev";
const STARTUP_NOTE: &str = "Note: Punch works best on WiFi. Mobile/corporate networks may fall back to relay.";

#[derive(Parser)]
#[command(
    name = "punch",
    about = "Punches through networks to connect two devices directly",
    version = "0.1.0",
    author = "Syed Mannan Saood"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Signalling server URL (for self-hosted instances)
    #[arg(long, global = true, default_value = DEFAULT_SERVER)]
    server: String,

    /// Enable verbose debug output
    #[arg(long, global = true, short = 'v')]
    verbose: bool,

    /// Enable local session logging to ~/.punch/logs/sessions.json
    #[arg(long, global = true)]
    log: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a connection code and wait for a peer
    Generate {
        /// Number of times this code can be used (Q-No mode)
        #[arg(long)]
        uses: Option<u32>,

        /// Create a permanent access token (P-No mode)
        #[arg(long)]
        permanent: bool,
    },

    /// Connect to a waiting peer using their code
    Connect {
        /// The connection code from the other device
        code: String,
    },

    /// Open the local session dashboard
    Dashboard,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(level)
        .without_time()
        .init();

    // Always show the startup note
    eprintln!("\n{}\n", STARTUP_NOTE);

    match cli.command {
        Commands::Generate { uses, permanent } => {
            cli::generate(cli.server, uses, permanent, cli.log).await?;
        }
        Commands::Connect { code } => {
            cli::connect(cli.server, code, cli.log).await?;
        }
        Commands::Dashboard => {
            cli::dashboard().await?;
        }
    }

    Ok(())
}