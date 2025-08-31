use crate::{
    btrfs,
    config::Config,
    database_operator::{DatabaseOperator, PostgresOperator},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

use crate::error::AppError;

#[derive(Parser)]
#[command(name = "dbranch")]
#[command(about = "🌿 dBranch 🌿 - PostgreSQL Database Branching System")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[clap(about = "Start dBranch proxy")]
    Start,
    #[clap(about = "Initialize a new dBranch project")]
    Init(InitArgs),
    #[clap(about = "Set the default branch project")]
    SetDefault(SetDefaultArgs),
    #[clap(about = "Create a new branch project")]
    Create(CreateArgs),
    #[clap(about = "List all branches projects")]
    List,
    #[clap(about = "Delete a branch project")]
    Delete(DeleteArgs),
    #[clap(about = "Delete a project")]
    DeleteProject(DeleteProjectArgs),
    #[clap(about = "Show details of a branch project")]
    Show(ShowArgs),
    #[clap(about = "Show the status of a project")]
    Status,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    #[arg(short, long, default_value = "dbranch_postgres")]
    name: String,

    #[arg(short, long, default_value = "5432")]
    port: u16,
}

#[derive(Args, Debug)]
pub struct SetDefaultArgs {
    name: String,
}

#[derive(Args, Debug)]
pub struct CreateArgs {
    name: String,

    #[arg(short, long)]
    source: Option<String>,
}

#[derive(Args, Debug)]
pub struct DeleteArgs {
    id: String,
}

#[derive(Args, Debug)]
pub struct DeleteProjectArgs {
    name: String,
}

#[derive(Args, Debug)]
pub struct ShowArgs {
    id: String,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub name: String,
    pub port: u16,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub active_branch: Option<String>,
    pub port: u16,
    pub created_at: DateTime<Utc>,
    pub branches: Vec<String>,
}

pub struct AppState {
    pub config: Config,
    pub active_project: Option<Project>,
    pub projects: Vec<Project>,
}

pub struct CliHandler {
    state: AppState,
}

impl CliHandler {
    pub fn new(state: AppState) -> Self {
        debug!(
            "Creating new CliHandler with {} projects",
            state.projects.len()
        );
        if let Some(ref active) = state.active_project {
            debug!("Active project: {}", active.name);
        }
        Self { state }
    }

    pub async fn handle_command(&mut self, cmd: Commands) -> Result<(), AppError> {
        debug!("Handling command: {:?}", cmd);
        match cmd {
            Commands::Start => {
                debug!("Start command received but should be handled in main");
                Err(AppError::Internal {
                    message: "Start command should be handled in main".into(),
                })
            }
            Commands::List => {
                info!("Listing all branch projects");
                debug!("Total projects: {}", self.state.projects.len());
                Err(AppError::NotImplemented {
                    command: "list".into(),
                })
            }
            Commands::Init(args) => {
                info!("Initializing dBranch instance: {}", args.name);
                debug!("Init args: name={}, port={}", args.name, args.port);

                if self.state.config.projects.contains(&args.name) {
                    debug!("Project {} already exists in config", args.name);
                    return Err(AppError::ProjectAlreadyExists { name: args.name });
                }

                let valid_port =
                    self.state
                        .config
                        .get_valid_port()
                        .ok_or(AppError::NoPortAvailable {
                            min: self.state.config.port_range.0,
                            max: self.state.config.port_range.1,
                        })?;

                let project = Project {
                    name: args.name.clone(),
                    path: Path::new(&self.state.config.path.clone()).join(args.name.clone()),
                    active_branch: None,
                    port: valid_port,
                    created_at: Utc::now(),
                    branches: Vec::new(),
                };
                debug!("Creating project at path: {:?}", project.path);

                // Mount disk
                {
                    debug!("Initializing BTRFS disk mount process");
                    let mut btrfs_operator =
                        btrfs::BtrfsOperator::new(project.path.clone(), self.state.config.clone());

                    debug!("Checking BTRFS installation");
                    btrfs_operator.check_btrfs().unwrap();
                    debug!("Reserving disk space");
                    btrfs_operator.reserve_space().unwrap();
                    debug!("Ensuring disk is unmounted before mounting");
                    let _ = btrfs_operator.unmount_disk();
                    debug!("Mounting BTRFS disk");
                    btrfs_operator.mount_disk().unwrap();
                    info!("BTRFS disk mounted successfully");
                }

                // Create Postgres
                {
                    debug!("Initializing PostgreSQL database creation");
                    let postgres_operator = PostgresOperator::new();

                    debug!(
                        "Finding available port in range {:?}",
                        self.state.config.port_range
                    );

                    info!("Found available port: {}", valid_port);

                    let db_name = format!("dbranch_{}", project.name.as_str());
                    debug!("Creating PostgreSQL database: {}", db_name);
                    postgres_operator
                        .create_database(
                            project.clone(),
                            self.state.config.clone(),
                            valid_port,
                            db_name.as_str(),
                        )
                        .await
                        .unwrap();
                    info!("PostgreSQL database created successfully");
                }

                debug!("Adding project to configuration");
                self.state.config.add_project(project);

                if self.state.config.default_project.is_none() {
                    debug!("No default project set, setting {} as default", args.name);
                    self.state
                        .config
                        .set_default_project(args.name.clone())
                        .unwrap();
                    info!("Set {} as default project", args.name);
                }

                info!("Project {} initialized successfully", args.name);
                Ok(())
            }
            Commands::Create(args) => {
                info!("Creating new branch project: {}", args.name);
                if let Some(ref source) = args.source {
                    debug!("Creating from source: {}", source);
                }

                if self.state.config.projects.contains(&args.name) {
                    debug!("Project {} already exists", args.name);
                    return Err(AppError::ProjectAlreadyExists { name: args.name });
                }
                debug!("Create command processed (implementation pending)");
                Ok(())
            }
            Commands::SetDefault(args) => {
                info!("Setting default project to: {}", args.name);
                self.state
                    .config
                    .set_default_project(args.name.clone())
                    .unwrap();
                debug!("Default project updated successfully");
                Ok(())
            }
            Commands::Delete(args) => {
                info!("Deleting branch project: {}", args.id);
                debug!("Delete command not yet implemented");
                Err(AppError::NotImplemented {
                    command: "delete".into(),
                })
            }
            Commands::DeleteProject(args) => {
                info!("Deleting project: {}", args.name);

                if !self.state.config.projects.contains(&args.name) {
                    debug!("Project {} not found in config", args.name);
                    return Err(AppError::ProjectNotFound { name: args.name });
                }

                let project = self
                    .state
                    .config
                    .get_project_info(Some(args.name.clone()))
                    .ok_or_else(|| AppError::ProjectNotFound {
                        name: args.name.clone(),
                    })?;

                debug!("Found project to delete: {:?}", project);

                // Delete PostgreSQL container
                {
                    debug!("Deleting PostgreSQL container");
                    let postgres_operator = PostgresOperator::new();
                    postgres_operator
                        .delete_database(project.clone(), &format!("dbranch_{}", project.name))
                        .await?;
                }

                // Delete BTRFS disk and unmount
                {
                    debug!("Cleaning up BTRFS disk");
                    let btrfs_operator =
                        crate::btrfs::BtrfsOperator::new(project.path, self.state.config.clone());

                    btrfs_operator.cleanup_disk().unwrap_or_else(|e| {
                        debug!("Failed to cleanup BTRFS disk: {}", e);
                    });
                }

                // Remove project from config and filesystem
                self.state.config.remove_project(&args.name)?;

                // If this was the default project, unset it
                if let Some(ref default) = self.state.config.default_project {
                    if default == &args.name {
                        debug!("Removing deleted project as default project");
                        self.state.config.default_project = None;
                        self.state.config.save_config();
                    }
                }

                info!("Project {} deleted successfully", args.name);
                Ok(())
            }
            Commands::Show(args) => {
                info!("Showing details for branch project: {}", args.id);
                debug!("Show command not yet implemented");
                Err(AppError::NotImplemented {
                    command: "show".into(),
                })
            }
            Commands::Status => {
                info!("Showing status of the active project");

                let project = self
                    .state
                    .config
                    .get_project_info(None)
                    .ok_or_else(|| AppError::DefaultProjectNotFound)?;

                debug!("Active project found: {}", project.name);
                println!("{}", String::from("-").repeat(60));
                println!("🌐 Project Name: {}", project.name);
                println!("⚙️  Project Path: {}", project.path.to_string_lossy());

                println!(
                    "🌿 Active Branch: {}",
                    project
                        .active_branch
                        .as_deref()
                        .unwrap_or("No active branch 🆓")
                );
                println!("{}", String::from("-").repeat(60));
                Ok(())
            }
        }
    }
}
