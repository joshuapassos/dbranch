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
    #[clap(about = "Use a specific branch")]
    Use(UseArgs),
    #[clap(about = "Stop all branches and containers")]
    Stop,
    #[clap(about = "Resume stopped branches and containers")]
    Resume,
    #[clap(about = "Restart the dBranch system after reboot")]
    Restart,
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
pub struct UseArgs {
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
    pub branches: Vec<Branch>,
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

                // Initialize individual BTRFS filesystem for this project
                {
                    debug!(
                        "Initializing individual BTRFS filesystem for project: {}",
                        args.name
                    );
                    let mut btrfs_operator =
                        btrfs::BtrfsOperator::new(project.clone(), self.state.config.clone());

                    debug!("Checking BTRFS installation");
                    btrfs_operator.check_btrfs().unwrap();

                    debug!("Reserving disk space for project filesystem");
                    btrfs_operator.reserve_space().unwrap();

                    debug!("Ensuring any existing mount is unmounted before mounting");
                    let _ = btrfs_operator.unmount_disk(); // Ignore errors - might not be mounted

                    debug!("Mounting project BTRFS filesystem");
                    btrfs_operator.mount_disk().unwrap();
                    info!(
                        "Project BTRFS filesystem '{}' mounted successfully",
                        args.name
                    );

                    info!("Project '{}' initialized with main subvolume", args.name);
                }

                // Create PostgreSQL database
                self.create_postgres(None, valid_port, &project).await;

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
                info!("Creating new branch project: {}", args.name.clone());
                if let Some(ref source) = args.source {
                    debug!("Creating from source: {}", source);
                }

                if self.state.config.projects.contains(&args.name) {
                    debug!("Project {} already exists", args.name);
                    return Err(AppError::ProjectAlreadyExists { name: args.name });
                }

                if let Some(ref active) = self.state.active_project {
                    if active
                        .branches
                        .iter()
                        .map(|b| b.name.clone())
                        .find(|name| *name == args.name)
                        .is_some()
                    {
                        debug!("Branch {} already exists", args.name);
                        return Err(AppError::BranchAlreadyExists { name: args.name });
                    }
                }

                let btrfs_operator = btrfs::BtrfsOperator::new(
                    self.state.active_project.clone().unwrap(),
                    self.state.config.clone(),
                );

                btrfs_operator.create_snapshot(&args.name).unwrap();

                let valid_port = self.state.config.get_valid_port().unwrap();

                // Create PostgreSQL database
                self.create_postgres(
                    Some(args.name.clone()),
                    valid_port,
                    &self.state.active_project.clone().unwrap(),
                )
                .await;

                self.state
                    .config
                    .create_branch(
                        self.state.active_project.clone().unwrap().name,
                        args.name.clone(),
                        valid_port,
                    )
                    .unwrap();

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

                let btrfs_operator =
                    crate::btrfs::BtrfsOperator::new(project.clone(), self.state.config.clone());

                let postgres_operator = PostgresOperator::new();

                for branch in project.clone().branches {
                    debug!("Deleting branch: {}", branch.name);

                    postgres_operator
                        .delete_database(project.clone(), &format!("{}", branch.name))
                        .await?;
                }

                // Delete PostgreSQL container
                {
                    debug!("Deleting PostgreSQL container");
                    postgres_operator
                        .delete_database(project.clone(), &format!("{}", project.name))
                        .await?;
                }

                // Delete BTRFS filesystem
                {
                    debug!("Cleaning up project BTRFS filesystem");

                    // Unmount and cleanup the entire project filesystem
                    btrfs_operator.unmount_disk().unwrap_or_else(|e| {
                        debug!("Failed to unmount BTRFS filesystem: {}", e);
                    });

                    btrfs_operator.cleanup_disk().unwrap_or_else(|e| {
                        debug!("Failed to cleanup BTRFS filesystem: {}", e);
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
            Commands::Use(args) => {
                info!("Switching to branch: {}", args.name);

                let project = self
                    .state
                    .config
                    .get_project_info(None)
                    .ok_or_else(|| AppError::DefaultProjectNotFound)?;

                debug!("Active project found: {}", project.name);

                if project
                    .branches
                    .iter()
                    .map(|b| b.name.clone())
                    .find(|name| *name == args.name)
                    .is_none()
                    && args.name != "main"
                {
                    debug!("Branch {} not found in active project", args.name);
                    return Err(AppError::BranchNotFound { name: args.name });
                }

                self.state
                    .config
                    .set_active_branch(project.name.clone(), args.name.clone())
                    .unwrap();

                info!("Switched to branch: {} successfully", args.name);
                Ok(())
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
            Commands::Stop => {
                info!("Stopping all branches and containers");

                for project in &self.state.projects {
                    debug!("Stopping containers for project: {}", project.name);

                    // Stop main project container
                    let postgres_operator = PostgresOperator::new();
                    let _ = postgres_operator
                        .stop_database(project.clone(), &project.name)
                        .await;

                    // Stop all branch containers
                    for branch in &project.branches {
                        debug!("Stopping branch container: {}", branch.name);
                        let _ = postgres_operator
                            .stop_database(project.clone(), &branch.name)
                            .await;
                    }

                    // Unmount BTRFS filesystem
                    debug!("Unmounting BTRFS filesystem for project: {}", project.name);
                    let btrfs_operator =
                        btrfs::BtrfsOperator::new(project.clone(), self.state.config.clone());
                    let _ = btrfs_operator.unmount_disk();
                }

                info!("All branches and containers stopped successfully");
                Ok(())
            }
            Commands::Resume => {
                info!("Resuming stopped branches and containers");

                for project in &self.state.projects {
                    debug!("Resuming project: {}", project.name);

                    // Mount BTRFS filesystem
                    let mut btrfs_operator =
                        btrfs::BtrfsOperator::new(project.clone(), self.state.config.clone());
                    if let Err(e) = btrfs_operator.mount_disk() {
                        debug!("Failed to mount BTRFS for project {}: {}", project.name, e);
                        continue;
                    }

                    // Start main project container
                    let postgres_operator = PostgresOperator::new();
                    let _ = postgres_operator
                        .create_database(
                            project.clone(),
                            self.state.config.clone(),
                            project.port,
                            &project.name,
                        )
                        .await;

                    // Start all branch containers
                    for branch in &project.branches {
                        debug!("Starting branch container: {}", branch.name);
                        let _ = postgres_operator
                            .create_database(
                                project.clone(),
                                self.state.config.clone(),
                                branch.port,
                                &branch.name,
                            )
                            .await;
                    }
                }

                info!("All branches and containers resumed successfully");
                Ok(())
            }
            Commands::Restart => {
                info!("Restarting dBranch system after reboot");

                // Stop all containers and unmount filesystems
                for project in &self.state.projects {
                    debug!("Stopping containers for project: {}", project.name);

                    // Stop main project container
                    let postgres_operator = PostgresOperator::new();
                    let _ = postgres_operator
                        .stop_database(project.clone(), &project.name)
                        .await;

                    // Stop all branch containers
                    for branch in &project.branches {
                        debug!("Stopping branch container: {}", branch.name);
                        let _ = postgres_operator
                            .stop_database(project.clone(), &branch.name)
                            .await;
                    }

                    // Unmount BTRFS filesystem
                    debug!("Unmounting BTRFS filesystem for project: {}", project.name);
                    let btrfs_operator =
                        btrfs::BtrfsOperator::new(project.clone(), self.state.config.clone());
                    let _ = btrfs_operator.unmount_disk();
                }

                // Resume all containers with proper mounts
                for project in &self.state.projects {
                    debug!("Resuming project: {}", project.name);

                    // Mount BTRFS filesystem
                    let mut btrfs_operator =
                        btrfs::BtrfsOperator::new(project.clone(), self.state.config.clone());
                    if let Err(e) = btrfs_operator.mount_disk() {
                        debug!("Failed to mount BTRFS for project {}: {}", project.name, e);
                        continue;
                    }

                    // Start main project container
                    let postgres_operator = PostgresOperator::new();
                    let _ = postgres_operator
                        .create_database(
                            project.clone(),
                            self.state.config.clone(),
                            project.port,
                            &project.name,
                        )
                        .await;

                    // Start all branch containers
                    for branch in &project.branches {
                        debug!("Starting branch container: {}", branch.name);
                        let _ = postgres_operator
                            .create_database(
                                project.clone(),
                                self.state.config.clone(),
                                branch.port,
                                &branch.name,
                            )
                            .await;
                    }
                }

                info!("dBranch system restarted successfully");
                Ok(())
            }
        }
    }

    async fn create_postgres(&mut self, name: Option<String>, valid_port: u16, project: &Project) {
        debug!("Initializing PostgreSQL database creation");
        let postgres_operator = PostgresOperator::new();
        debug!(
            "Finding available port in range {:?}",
            self.state.config.port_range
        );
        info!("Found available port: {}", valid_port);
        let db_name = name.unwrap_or_else(|| project.name.clone());
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
}
