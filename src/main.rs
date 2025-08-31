mod btrfs;
mod cli;
mod config;
mod database_operator;
mod error;

use crate::{
    cli::{AppState, Commands},
    config::Config,
};
use anyhow::Result;
use clap::Parser;
use cli::Cli;
use tracing::{debug, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    debug!("CLI arguments parsed: {:?}", cli.command);

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("debug"))
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    debug!("Tracing subscriber initialized with debug level");

    info!("🌿 dBranch - PostgreSQL Database Branching System");

    debug!("Loading configuration from file...");
    let config = Config::from_file().unwrap();
    info!("Configuration loaded successfully");

    debug!("Creating CLI handler with initial state");
    let mut cli_handler = cli::CliHandler::new(AppState {
        config,
        active_project: None,
        projects: vec![],
    });
    debug!("CLI handler initialized");

    debug!("Processing command: {:?}", cli.command);
    match cli.command {
        Commands::Start => {
            info!("Starting dBranch service...");
            debug!("Initializing server components");
            run_server().unwrap();
            info!("dBranch service started successfully");
        }
        cmd => {
            debug!("Delegating command to CLI handler");
            cli_handler.handle_command(cmd).await.unwrap();
            debug!("Command processed successfully");
        }
    }
}

fn run_server() -> Result<(), error::AppError> {
    debug!("Server startup initiated");
    // TODO: Implement server logic
    info!("Server is now running (placeholder)");
    Ok(())
}
