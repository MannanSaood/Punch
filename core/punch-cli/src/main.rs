mod cli;

use punch_core::{
    signaling,
    punch,
    token,
    token_store,
    dashboard_server,
    transfer,
    safety,
    forward,
    shell,
    pipe,
};

use clap::{Parser, Subcommand};

const DEFAULT_SERVER: &str = "wss://129.159.21.6.nip.io";
const STARTUP_NOTE: &str = "Note: Punch works best on WiFi. Mobile/corporate networks may fall back to relay.";

#[derive(Parser)]
#[command(
    name = "punch",
    about = "Punches through networks to connect two devices directly",
    version = "0.8.0",
    author = "Syed Mannan Saood"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, global = true, default_value = DEFAULT_SERVER)]
    server: String,

    #[arg(long, global = true, short = 'v')]
    verbose: bool,

    #[arg(long, global = true)]
    log: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new connection code and wait for a peer
    Generate {
        /// Number of uses before token expires (Q-No mode)
        #[arg(long)]
        uses: Option<u32>,

        /// Create a permanent access token (P-No mode)
        #[arg(long)]
        permanent: bool,
    },

    /// Wait for a peer on an existing token code (no use consumed until peer connects)
    Listen {
        /// Existing token code to listen on
        code: String,
    },

    /// Connect to a waiting peer using their code
    Connect {
        /// The connection code from the other device
        code: String,
    },

    /// Verify a permanent (P-No) token before first use
    Verify {
        /// The token code to verify
        code: String,
    },

    /// Revoke a token so it can no longer be used
    Revoke {
        /// The token code to revoke
        code: String,
    },

    /// Port forwarding (expose a port, or connect to a peer's port)
    Forward {
        #[command(subcommand)]
        action: ForwardAction,
    },

    /// Remote terminal access
    Shell {
        #[command(subcommand)]
        action: ShellAction,
    },

    /// Stream stdin to a remote peer, or receive stdout
    Pipe {
        #[command(subcommand)]
        action: PipeAction,
    },

    /// Send a file to a peer
    Send {
        /// Path to the file to send
        file: String,
    },

    /// Receive a file from a peer
    Receive {
        /// The connection code from the sender
        code: String,
        /// Destination directory (default: current directory)
        #[arg(long, short, default_value = ".")]
        dest: String,
    },

    /// List all active tokens and their status
    Tokens,

    /// Open the local session dashboard
    Dashboard,
}

#[derive(Subcommand)]
enum ForwardAction {
    /// Expose a local port to a peer (run where the service listens)
    Expose {
        /// Local port to expose
        port: u16,
        /// Enable UDP forwarding in addition to TCP
        #[arg(long)]
        udp: bool,
        /// Number of uses before token expires (Q-No)
        #[arg(long)]
        uses: Option<u32>,
        /// Create a permanent access token (P-No)
        #[arg(long)]
        permanent: bool,
    },
    /// Connect to a peer's forwarded port (run on the client machine)
    Connect {
        /// Connection code from the exposer
        code: String,
        /// Local port to listen on (auto-assigned if omitted)
        #[arg(long, short)]
        local: Option<u16>,
        /// Enable UDP (must match exposer)
        #[arg(long)]
        udp: bool,
    },
}

#[derive(Subcommand)]
enum ShellAction {
    /// Allow a remote client to access a shell on this machine (run on Device B)
    Host {
        /// Number of uses before token expires (Q-No)
        #[arg(long)]
        uses: Option<u32>,
        /// Create a permanent access token (P-No)
        #[arg(long)]
        permanent: bool,
    },
    /// Connect to a remote shell (run on Device A)
    Connect {
        /// Connection code from the host
        code: String,
    },
}

#[derive(Subcommand)]
enum PipeAction {
    /// Send stdin to a peer
    Send,
    /// Receive stdout from a peer
    Receive {
        /// Connection code from the sender
        code: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(level)
        .without_time()
        .init();

    match &cli.command {
        Commands::Generate { .. } | Commands::Connect { .. } | Commands::Listen { .. } => {
            eprintln!("\n{}\n", STARTUP_NOTE);
        }
        _ => {}
    }

    match cli.command {
        Commands::Generate { uses, permanent } => {
            cli::generate(cli.server, uses, permanent, cli.log).await?;
        }
        Commands::Listen { code } => {
            cli::listen(cli.server, code, cli.log).await?;
        }
        Commands::Connect { code } => {
            cli::connect(cli.server, code, cli.log).await?;
        }
        Commands::Verify { code } => {
            cli::verify(code).await?;
        }
        Commands::Revoke { code } => {
            cli::revoke(code).await?;
        }
        Commands::Forward { action } => {
            eprintln!("\n{}\n", STARTUP_NOTE);
            match action {
                ForwardAction::Expose {
                    port,
                    udp,
                    uses,
                    permanent,
                } => {
                    cli::forward_expose(cli.server, port, udp, uses, permanent).await?;
                }
                ForwardAction::Connect { code, local, udp } => {
                    cli::forward_connect(cli.server, code.clone(), local, udp).await?;
                }
            }
        }
        Commands::Shell { action } => {
            eprintln!("\n{}\n", STARTUP_NOTE);
            match action {
                ShellAction::Host { uses, permanent } => {
                    cli::shell_host(cli.server, uses, permanent).await?;
                }
                ShellAction::Connect { code } => {
                    cli::shell_connect(cli.server, code.clone()).await?;
                }
            }
        }
        Commands::Pipe { action } => {
            eprintln!("\n{}\n", STARTUP_NOTE);
            match action {
                PipeAction::Send => {
                    cli::pipe_send(cli.server).await?;
                }
                PipeAction::Receive { code } => {
                    cli::pipe_recv(cli.server, code.clone()).await?;
                }
            }
        }
        Commands::Send { file } => {
            eprintln!("\n{}\n", STARTUP_NOTE);
            cli::send(cli.server, file.clone(), cli.log).await?;
        }
        Commands::Receive { code, dest } => {
            eprintln!("\n{}\n", STARTUP_NOTE);
            cli::receive(cli.server, code.clone(), dest.clone()).await?;
        }
        Commands::Tokens => {
            cli::tokens().await?;
        }
        Commands::Dashboard => {
            cli::dashboard().await?;
        }
    }

    Ok(())
}