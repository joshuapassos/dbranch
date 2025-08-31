mod btrfs;
mod cli;
mod config;
mod database_operator;
mod error;

use crate::{
    cli::{AppState, Commands, Project},
    config::Config,
};
use anyhow::Result;
use clap::Parser;
use cli::Cli;
use tokio::{
    io,
    net::{TcpListener, TcpStream},
};
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

    let project = config.get_project_info(config.default_project.clone());

    debug!("Creating CLI handler with initial state");
    let mut cli_handler = cli::CliHandler::new(AppState {
        config: config.clone(),
        active_project: project.clone(),
        projects: config.get_projects(),
    });
    debug!("CLI handler initialized");

    debug!("Processing command: {:?}", cli.command);
    match cli.command {
        Commands::Start => {
            info!("Starting dBranch service...");
            debug!("Initializing server components");
            run_server(config, project.unwrap()).await.unwrap();
            info!("dBranch service started successfully");
        }
        cmd => {
            debug!("Delegating command to CLI handler");
            cli_handler.handle_command(cmd).await.unwrap();
            debug!("Command processed successfully");
        }
    }
}

async fn run_server(config: Config, project: Project) -> Result<(), error::AppError> {
    debug!("Server startup initiated");
    let bind_addr = format!("0.0.0.0:{}", config.proxy_port);
    info!("🚀 Proxy PostgreSQL starting...");
    info!("📡 Listening on: {}", bind_addr);
    let listener = TcpListener::bind(&bind_addr).await.unwrap();

    while let Ok((client, addr)) = listener.accept().await {
        println!("🔗 New connection from: {}", addr);

        let target = format!("localhost:{}", project.port);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(client, &target).await {
                println!("❌ Connection error {}: {}", addr, e);
            } else {
                println!("✅ Connection {} finished", addr);
            }
        });
    }

    Ok(())
}

async fn handle_connection(mut client: TcpStream, target_addr: &str) -> io::Result<()> {
    let mut server = TcpStream::connect(target_addr).await?;

    let (mut client_read, mut client_write) = client.split();
    let (mut server_read, mut server_write) = server.split();

    let client_to_server = io::copy(&mut client_read, &mut server_write);
    let server_to_client = io::copy(&mut server_read, &mut client_write);

    tokio::try_join!(client_to_server, server_to_client)?;

    Ok(())
}
