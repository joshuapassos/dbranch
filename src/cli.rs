use crate::config::DEFAULT_CONFIG_PATH;
use crate::error::AppError;
use crate::fiemap::{FolderInfo, get_folder_size};
use crate::snapshot;
use crate::{
    config::Config,
    database_operator::{DatabaseOperator, PostgresOperator},
};
use anyhow::Result;
use chrono::Utc;
use clap::{Args, Parser, Subcommand};
use prettytable::{Attr, Cell, Row, Table};
use rustix::path::Arg;
use size::Size;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

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
    #[clap(about = "Initialize a PostgreSQL database")]
    InitPostgres,
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

pub struct AppState {
    pub config: Config,
}

pub struct CliHandler {
    state: AppState,
}

impl CliHandler {
    pub fn new(state: AppState) -> Self {
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
                Err(AppError::NotImplemented {
                    command: "list".into(),
                })
            }
            Commands::Init(args) => {
                info!("Initializing dBranch instance: {}", args.name);
                debug!("Init args: name={}, port={}", args.name, args.port);

                // Initialize individual BTRFS filesystem for this project
                {
                    debug!(
                        "Initializing individual BTRFS filesystem for project: {}",
                        args.name
                    );

                    info!("Project '{}' initialized with main subvolume", args.name);
                }

                debug!("Adding project to configuration");
                self.state.config.name = args.name.clone();

                self.state.config.save_config();

                info!("Project {} initialized successfully", args.name);
                Ok(())
            }
            Commands::InitPostgres => {
                info!("Initializing standalone PostgreSQL database");

                self.create_postgres(None, self.state.config.get_valid_port().unwrap())
                    .await;

                info!("Standalone PostgreSQL database initialized successfully");
                Ok(())
            }
            Commands::Create(args) => {
                info!("Creating new branch project: {}", args.name.clone());
                if let Some(ref source) = args.source {
                    debug!("Creating from source: {}", source);
                }

                let project_name = self.state.config.name.clone();

                let src_path = Path::new(&self.state.config.mount_point)
                    .join(&project_name.clone())
                    .join("main/data");

                let dest_path = Path::new(&self.state.config.mount_point)
                    .join(&project_name.clone())
                    .join(&args.name)
                    .join("data");

                info!(
                    "Copying data from {:?} to {:?}",
                    src_path.clone(),
                    dest_path.clone()
                );

                snapshot::snapshot(&src_path, &dest_path).unwrap();

                let valid_port = self.state.config.get_valid_port().unwrap();

                // Create PostgreSQL database
                self.create_postgres(Some(args.name.clone()), valid_port)
                    .await;

                self.state
                    .config
                    .create_branch(args.name.clone(), valid_port);

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

                if self.state.config.name != args.name {
                    debug!("Project {} not found in config", args.name);
                    return Err(AppError::ProjectNotFound { name: args.name });
                }

                let postgres_operator = PostgresOperator::new();

                for branch in self
                    .state
                    .config
                    .branches
                    .iter()
                    .filter(|b| !b.is_main)
                    .collect::<Vec<&crate::config::Branch>>()
                {
                    debug!("Deleting branch: {}", branch.name);

                    let _ = postgres_operator
                        .delete_database(self.state.config.clone(), branch.name.as_str())
                        .await;
                }

                self.state.config.branches.clear();

                self.state.config.save_config();

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

                self.state
                    .config
                    .set_active_branch(args.name.clone())
                    .unwrap();

                info!("Switched to branch: {} successfully", args.name);
                Ok(())
            }
            Commands::Status => {
                info!("Showing status of the project");

                let postgres_operator = PostgresOperator::new();

                println!("{}", String::from("=").repeat(80));
                println!("PROJECT: {}", self.state.config.name);
                println!("{}", String::from("-").repeat(80));
                println!("Path: {}", DEFAULT_CONFIG_PATH.to_string_lossy());
                println!(
                    "🌿 Active Branch: {}",
                    self.state.config.active_branch.as_deref().unwrap_or("none")
                );

                let main_branch = self
                    .state
                    .config
                    .branches
                    .iter()
                    .find(|p| p.is_main)
                    .map(|b| {
                        (
                            Path::new(&self.state.config.mount_point).join(&b.name),
                            get_folder_size(
                                &Path::new(&self.state.config.mount_point)
                                    .join(self.state.config.name.clone())
                                    .join(&b.name),
                            )
                            .unwrap(),
                        )
                    })
                    .unwrap();

                let branches: Vec<(PathBuf, FolderInfo)> = self
                    .state
                    .config
                    .branches
                    .iter()
                    .filter(|p| !p.is_main)
                    .map(|b| {
                        (
                            Path::new(&self.state.config.mount_point).join(&b.name),
                            get_folder_size(
                                &Path::new(&self.state.config.mount_point)
                                    .join(self.state.config.name.clone())
                                    .join(&b.name),
                            )
                            .unwrap(),
                        )
                    })
                    .collect();

                println!("{}", String::from("-").repeat(80));

                let mut table = Table::new();

                table.add_row(Row::new(vec![
                    Cell::new("Branch").with_style(Attr::Bold),
                    Cell::new("Logical Size").with_style(Attr::Bold),
                    Cell::new("Unique Data").with_style(Attr::Bold),
                    Cell::new("Container").with_style(Attr::Bold),
                    Cell::new("Age").with_style(Attr::Bold),
                ]));

                let main_container_status = postgres_operator
                    .is_container_running(format!("{}_main", self.state.config.name).as_str())
                    .await
                    .unwrap_or(false);

                let main_age = {
                    let duration = Utc::now() - self.state.config.created_at;
                    if duration.num_days() > 0 {
                        format!("{}d", duration.num_days())
                    } else if duration.num_hours() > 0 {
                        format!("{}h", duration.num_hours())
                    } else {
                        format!("{}m", duration.num_minutes())
                    }
                };

                // table.add_row(Row::new(vec![
                //     Cell::new("📦 Shared Base"),
                //     Cell::new(&Size::from_bytes(main_branch.1.shared_size).to_string()),
                //     Cell::new("-"),
                //     Cell::new("🔗 Shared"),
                //     Cell::new("-"),
                // ]));

                table.add_row(Row::new(vec![
                    Cell::new("main").with_style(Attr::Bold),
                    Cell::new(
                        Size::from_bytes(main_branch.1.logical_size)
                            .to_string()
                            .as_str(),
                    ),
                    Cell::new(
                        Size::from_bytes(main_branch.1.logical_size - main_branch.1.shared_size)
                            .to_string()
                            .as_str(),
                    ),
                    Cell::new(if main_container_status {
                        "✅ Running"
                    } else {
                        "❌ Stopped"
                    }),
                    Cell::new(main_age.as_str()),
                ]));

                for branch in branches {
                    let branch_name = branch.0.file_name().unwrap().to_string_lossy().to_string();

                    let container_status = postgres_operator
                        .is_container_running(
                            format!("{}_{}", self.state.config.name, branch_name).as_str(),
                        )
                        .await
                        .unwrap_or(false);

                    let age = {
                        let duration = Utc::now()
                            - self
                                .state
                                .config
                                .branches
                                .iter()
                                .find(|b| b.name == branch_name)
                                .unwrap()
                                .created_at;
                        if duration.num_days() > 0 {
                            format!("{}d", duration.num_days())
                        } else if duration.num_hours() > 0 {
                            format!("{}h", duration.num_hours())
                        } else {
                            format!("{}m", duration.num_minutes())
                        }
                    };

                    table.add_row(Row::new(vec![
                        Cell::new(branch_name.as_str()),
                        Cell::new(Size::from_bytes(branch.1.logical_size).to_string().as_str()),
                        Cell::new(
                            Size::from_bytes(branch.1.logical_size - branch.1.shared_size)
                                .to_string()
                                .as_str(),
                        ),
                        Cell::new(if container_status {
                            "✅ Running"
                        } else {
                            "❌ Stopped"
                        }),
                        Cell::new(age.as_str()),
                    ]));
                }

                let _ = table.print_tty(true);

                println!("{}", String::from("=").repeat(80));
                Ok(())
            }
            Commands::Stop => {
                info!("Stopping all branches and containers");

                debug!(
                    "Stopping containers for project: {}",
                    self.state.config.name
                );

                let postgres_operator = PostgresOperator::new();

                for branch in &self.state.config.branches {
                    debug!("Stopping branch container: {}", branch.name);
                    let _ = postgres_operator
                        .stop_database(self.state.config.clone(), &branch.name)
                        .await;
                }
                let _ = postgres_operator
                    .stop_database(self.state.config.clone(), &self.state.config.name)
                    .await;

                debug!(
                    "Unmounting BTRFS filesystem for project: {}",
                    self.state.config.name
                );

                info!("All branches and containers stopped successfully");
                Ok(())
            }
            Commands::Resume => {
                info!("Resuming stopped branches and containers");

                debug!("Resuming project: {}", self.state.config.name);

                let postgres_operator = PostgresOperator::new();
                let _ = postgres_operator
                    .create_database(
                        self.state.config.clone(),
                        self.state.config.get_valid_port().unwrap(),
                        "main",
                    )
                    .await;

                for branch in &self.state.config.branches {
                    debug!("Starting branch container: {}", branch.name);
                    let _ = postgres_operator
                        .create_database(self.state.config.clone(), branch.port, &branch.name)
                        .await;
                }

                info!("All branches and containers resumed successfully");
                Ok(())
            }
        }
    }

    async fn create_postgres(&mut self, name: Option<String>, valid_port: u16) {
        debug!("Initializing PostgreSQL database creation");
        let postgres_operator = PostgresOperator::new();
        debug!(
            "Finding available port in range {:?}, {:?}",
            self.state.config.port_min, self.state.config.port_max
        );
        info!("Found available port: {}", valid_port);
        let db_name = name.unwrap_or_else(|| "main".to_string());
        debug!("Creating PostgreSQL database: {}", db_name);
        postgres_operator
            .create_database(self.state.config.clone(), valid_port, db_name.as_str())
            .await
            .unwrap();
        info!("PostgreSQL database created successfully");
    }
}
