mod cli;
mod config;
mod services;

use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use cli::{Cli, Commands};
use config::Config;
use dotenvy::dotenv;
use std::io;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let cli = Cli::parse();

    // Load config to get default values if not overridden by flags/env
    let config = Config::load()?;

    // Determine the AE address: Flag/Env > Config > Default
    let active_context = config.get_active_context();
    let ae_addr = cli.ae_addr.unwrap_or_else(|| {
        active_context
            .map(|c| c.ae_addr.clone())
            .unwrap_or_else(|| "http://[::1]:50051".to_string())
    });

    let hq_addr = cli.hq_addr.unwrap_or_else(|| {
        active_context
            .map(|c| c.hq_addr.clone())
            .unwrap_or_else(|| "http://127.0.0.1:50052".to_string())
    });

    let hq_admin_token = cli
        .hq_admin_token
        .or_else(|| active_context.and_then(|c| c.hq_admin_token.clone()));

    println!("Using AE address: {}", ae_addr);
    println!("Using HQ address: {}", hq_addr);

    match cli.command {
        Commands::AudioEngine(cmd) => {
            services::audio_engine::handle_command(ae_addr, cmd).await?;
        }
        Commands::Hq(cmd) => {
            services::hq::handle_command(hq_addr, hq_admin_token, cmd).await?;
        }
        Commands::Config(cmd) => {
            services::config::handle_command(cmd)?;
        }
        Commands::Completion { shell } => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            generate(shell, &mut cmd, bin_name, &mut io::stdout());
        }
    }

    Ok(())
}
