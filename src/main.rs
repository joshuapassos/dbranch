mod btrfs;
mod cli;
mod config;
mod copy_ref;
mod database_operator;
mod error;
mod copy_ref;

use std::sync::Arc;

use crate::{
    cli::{AppState, Commands, Project},
    config::Config,
    error::AppError,
};
use anyhow::Result;
use clap::Parser;
use cli::Cli;
use tokio::{
    io,
    net::{TcpListener, TcpStream},
    sync::RwLock,
};
use tracing::{debug, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    debug!("CLI arguments parsed: {:?}", cli.command);

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("INFO"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    debug!("Tracing subscriber initialized with debug level");

    info!("🌿 dBranch - PostgreSQL Database Branching System");

    debug!("Loading configuration from file...");

    let config = Arc::new(RwLock::new(Config::from_file().unwrap()));

    let project = Arc::new(RwLock::new(
        config
            .read()
            .await
            .get_project_info(config.read().await.default_project.clone()),
    ));

    tokio::spawn(sync_config(config.clone(), project.clone()));

    info!("Configuration loaded successfully");

    debug!("Creating CLI handler with initial state");
    let mut cli_handler = cli::CliHandler::new(AppState {
        config: config.read().await.clone(),
        active_project: project.read().await.clone(),
        projects: config.read().await.get_projects(),
    });
    debug!("CLI handler initialized");

    debug!("Processing command: {:?}", cli.command);
    match cli.command {
        Commands::Start => {
            info!("Starting dBranch service...");
            debug!("Initializing server components");
            run_server(config, project).await.unwrap();
            info!("dBranch service started successfully");
        }
        cmd => {
            debug!("Delegating command to CLI handler");
            cli_handler.handle_command(cmd).await.unwrap();
            debug!("Command processed successfully");
        }
    }
}

async fn sync_config(config: Arc<RwLock<Config>>, project: Arc<RwLock<Option<Project>>>) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        match Config::from_file() {
            Ok(new_config) => {
                config.write().await.clone_from(&new_config);
                *project.write().await =
                    new_config.get_project_info(new_config.default_project.clone());
            }
            Err(e) => {
                AppError::Internal {
                    message: format!("Failed to reload configuration: {}", e),
                };
            }
        }
    }
}

async fn run_server(
    config: Arc<RwLock<Config>>,
    project: Arc<RwLock<Option<Project>>>,
) -> Result<(), error::AppError> {
    if project.read().await.is_none() {
        return Err(AppError::Internal {
            message: "No active project found.".to_string(),
        });
    }

    debug!("Server startup initiated");
    let bind_addr = format!("0.0.0.0:{}", config.read().await.proxy_port);
    info!("🚀 Proxy PostgreSQL starting...");
    info!("📡 Listening on: {}", bind_addr);
    println!(
        "🔄 ({}) Forwarding to branch '{}'",
        project.read().await.clone().unwrap().name,
        project
            .read()
            .await
            .clone()
            .unwrap()
            .active_branch
            .clone()
            .unwrap_or("main".to_string())
    );
    println!("🚀 Proxy PostgreSQL starting...");
    println!("📡 Listening on: {}", bind_addr);
    let listener = TcpListener::bind(&bind_addr).await.unwrap();

    while let Ok((client, addr)) = listener.accept().await {
        println!("🔗 New connection from: {}", addr);

        let target_port = match &project.read().await.clone().unwrap().active_branch {
            Some(branch_name) => {
                println!(
                    "🔄 ({}) Forwarding to branch '{}'",
                    project.read().await.clone().unwrap().name,
                    branch_name
                );
                project
                    .read()
                    .await
                    .clone()
                    .unwrap()
                    .branches
                    .iter()
                    .find(|b| b.name == *branch_name)
                    .map(|b| b.port)
                    .unwrap()
            }
            None => {
                println!(
                    "🔄 ({}) Forwarding to branch 'main'",
                    project.read().await.clone().unwrap().name
                );
                project.read().await.clone().unwrap().port
            }
        };

        let target = format!("localhost:{}", target_port);
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
