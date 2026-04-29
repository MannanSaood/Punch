use crate::token::{Token, TokenType};
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

    // P-No requires extra confirmation
    if token.token_type == TokenType::PNo {
        println!("⚠️  Permanent access token — this peer will always be able to connect.");
        println!("   Are you sure? (yes/no): ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() != "yes" {
            println!("Cancelled.");
            return Ok(());
        }
    }

    println!("\n{}: {}", token.display_label(), token.code);
    println!("Token type: {}", token.token_type);
    println!("\nWaiting for peer...\n");

    let mut client = SignalingClient::connect(&server, &token.code).await?;
    client.wait_for_peer().await?;

    let engine = PunchEngine::new();
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

/// Handle `punch dashboard`
pub async fn dashboard() -> anyhow::Result<()> {
    let log_path = crate::logger::log_path();

    if !log_path.exists() {
        println!("No session logs found yet.");
        println!("Run `punch generate --log` or `punch connect --log` to start logging.");
        return Ok(());
    }

    println!("Opening Punch dashboard at http://localhost:7777");
    println!("Press Ctrl+C to stop.\n");

    // In v0.4: serve the compiled Svelte dashboard here
    // For now: placeholder
    println!("Dashboard coming in v0.4. Your logs are at: {}", log_path.display());

    Ok(())
}
