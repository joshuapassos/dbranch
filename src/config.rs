use std::{
    fs::{self, File},
    io::BufWriter,
    net::TcpListener,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::{cli::Project, error::AppError};

#[derive(Clone)]
pub struct FolderConfig {
    path: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]

pub struct FileConfigInfo {
    pub api_port: Option<u16>,
    pub proxy_port: Option<u16>,
    pub port_min: Option<u16>,
    pub port_max: Option<u16>,
    pub mount_point: Option<String>,
    pub default_project: Option<String>,
    pub postgres_config: Option<PostgresConfig>,
    pub projects: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub path: PathBuf,
    pub config_path: PathBuf,
    pub api_port: u16,
    pub proxy_port: u16,
    pub port_range: (u16, u16),
    pub mount_point: String,
    pub default_project: Option<String>,
    pub postgres_config: PostgresConfig,
    pub projects: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]

pub struct PostgresConfig {
    pub user: String,
    pub password: String,
    pub database: Option<String>,
}

impl FolderConfig {
    pub fn from_env() -> Result<Self> {
        let config_path = std::env::var("DBRANCH_CONFIG").unwrap_or(".config".to_string());
        debug!("Loading folder config from path: {}", config_path);
        Ok(Self {
            path: config_path.parse()?,
        })
    }
}

impl Config {
    pub fn get_valid_port(&self) -> Option<u16> {
        debug!(
            "Searching for available port in range {}-{}",
            self.port_range.0, self.port_range.1
        );
        for port in self.port_range.0..=self.port_range.1 {
            match TcpListener::bind(("127.0.0.1", port)) {
                Ok(_) => {
                    debug!("Found available port: {}", port);
                    return Some(port);
                }
                Err(_) => continue,
            }
        }
        debug!(
            "No available ports found in range {}-{}",
            self.port_range.0, self.port_range.1
        );
        None
    }
    pub fn from_env(path: &Path, c: FileConfigInfo) -> Result<Self, AppError> {
        debug!("Creating config from environment and file data");
        debug!("Config path: {:?}", path);
        Ok(Self {
            path: path.to_path_buf(),
            config_path: path.join("dbranch.config.json"),
            api_port: std::env::var("DBRANCH_API_PORT")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .or(c.api_port)
                .unwrap_or(8080),
            proxy_port: std::env::var("DBRANCH_PROXY_PORT")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .or(c.proxy_port)
                .unwrap_or(5432),
            port_range: (
                std::env::var("DBRANCH_PORT_START")
                    .ok()
                    .and_then(|v| v.parse::<u16>().ok())
                    .or(c.port_min)
                    .unwrap_or(7000),
                std::env::var("DBRANCH_PORT_END")
                    .ok()
                    .and_then(|v| v.parse::<u16>().ok())
                    .or(c.port_max)
                    .unwrap_or(7999),
            ),
            mount_point: std::env::var("DBRANCH_MOUNT_POINT")
                .ok()
                .and_then(|v| v.parse().ok())
                .or(c.mount_point)
                .unwrap_or("/mnt/dbranch".to_string()),
            projects: c.projects.unwrap_or(vec![]),
            postgres_config: c.postgres_config.unwrap_or(PostgresConfig {
                user: "dbranch_user".into(),
                password: "dbbranch_pass".into(),
                database: None,
            }),
            default_project: std::env::var("DBRANCH_DEFAULT_PROJECT")
                .ok()
                .and_then(|v| v.parse().ok())
                .or(c.default_project),
        })
    }

    pub fn from_file() -> Result<Self, AppError> {
        debug!("Loading configuration from file");
        let folder_config = FolderConfig::from_env().unwrap();

        let file_config = Path::new(&folder_config.path).join("dbranch.config.json");
        debug!("Config file path: {:?}", file_config);

        let (needs_create, json_string) =
            match fs::read_to_string(file_config.clone().as_mut_os_string()) {
                Ok(content) => {
                    debug!("Config file exists, reading content");
                    (false, content)
                }
                Err(_) => {
                    debug!("Config file doesn't exist, will create with defaults");
                    (true, "{}".to_string())
                }
            };

        let json = serde_json::from_str::<FileConfigInfo>(json_string.as_str()).map_err(|e| {
            AppError::Internal {
                message: format!("Failed to parse config file {}", e),
            }
        });

        let parsed_config = json
            .and_then(|op| Config::from_env(Path::new(&folder_config.path), op))
            .map_err(|e| AppError::Internal {
                message: format!("Failed to read config file: {}", e),
            });

        if needs_create {
            info!("Creating new config file at {:?}", file_config);
            let file = File::create(file_config.clone()).map_err(|e| AppError::Internal {
                message: format!(
                    "Failed to create config file {:?}: {}",
                    file_config.as_os_str(),
                    e
                ),
            })?;

            let obj = FileConfigInfo {
                api_port: Some(parsed_config.as_ref().unwrap().api_port),
                proxy_port: Some(parsed_config.as_ref().unwrap().proxy_port),
                port_min: Some((parsed_config.as_ref().unwrap().port_range).0),
                port_max: Some((parsed_config.as_ref().unwrap().port_range).1),
                mount_point: Some(parsed_config.as_ref().unwrap().mount_point.clone()),
                default_project: parsed_config.as_ref().unwrap().default_project.clone(),
                postgres_config: Some(PostgresConfig {
                    user: parsed_config.as_ref().unwrap().postgres_config.user.clone(),
                    password: parsed_config
                        .as_ref()
                        .unwrap()
                        .postgres_config
                        .password
                        .clone(),
                    database: parsed_config
                        .as_ref()
                        .unwrap()
                        .postgres_config
                        .database
                        .clone(),
                }),
                projects: Some(vec![]),
            };

            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, &obj).map_err(|e| AppError::Internal {
                message: format!("Failed to write config file {:?}: {}", file_config, e),
            })?;
            info!("Config file created successfully");
        } else {
            debug!("Using existing config file");
        }

        return parsed_config;
    }

    pub fn set_default_project(&mut self, project: String) -> Result<(), AppError> {
        debug!("Setting default project to: {}", project);
        if self.projects.iter().find(|&p| p == &project).is_none() {
            debug!("Project {} not found in project list", project);
            return Err(AppError::Internal {
                message: format!("Project {} does not exist", project),
            });
        }

        self.default_project = Some(project.clone());
        debug!("Saving config with new default project");
        self.save_config();
        info!("Default project set to: {}", project);
        Ok(())
    }

    pub fn get_project_info(&self, name: Option<String>) -> Option<Project> {
        let project_name = match name {
            Some(n) => n,
            None => {
                if self.default_project.is_none() {
                    debug!("No default project set");
                    return None;
                }
                self.default_project.as_ref().unwrap().clone()
            }
        };

        let project_path = self.path.join(project_name);
        let metadata_path = project_path.join("metadata.json");

        debug!("Looking for metadata file at: {:?}", metadata_path);

        let metadata_content = fs::read_to_string(&metadata_path).ok()?;
        let project: Project = serde_json::from_str(&metadata_content).ok()?;
        debug!(
            "Project info retrieved successfully for: {}",
            self.default_project.as_ref().unwrap()
        );
        Some(project)
    }

    pub fn get_projects(&self) -> Vec<Project> {
        self.projects
            .iter()
            .filter_map(|name| self.get_project_info(Some(name.clone())))
            .collect()
    }

    pub fn add_project(&mut self, project: Project) {
        info!("Adding new project: {}", project.name);
        debug!("Project path: {:?}", project.path);
        self.create_project(project.clone()).unwrap();
        self.projects.push(project.name.clone());
        debug!("Total projects now: {}", self.projects.len());
        self.save_config();
        info!("Project {} added successfully", project.name);
    }

    fn create_project(&self, project: Project) -> Result<(), AppError> {
        let project_dir = Path::new(&self.path).join(project.name.clone());
        debug!("Creating project directory: {:?}", project_dir);

        match fs::create_dir(&project_dir) {
            Ok(_) => {
                info!("Project directory created successfully.");
            }
            Err(e) => {
                debug!(
                    "Failed to create project directory: {:?} - {} Ignoring...",
                    project_dir, e
                );
            }
        }

        let metadata_path = Path::new(&self.path)
            .join(project.name.clone())
            .join("metadata.json");
        debug!("Creating metadata file: {:?}", metadata_path);

        let metadata = File::create(metadata_path).map_err(|e| AppError::Internal {
            message: format!("Failed to create metadata file: {}", e),
        })?;

        let mut writer = BufWriter::new(metadata);
        serde_json::to_writer_pretty(&mut writer, &project).map_err(|e| AppError::Internal {
            message: format!("Failed to write metadata file: {}", e),
        })?;
        debug!("Metadata file written successfully");

        Ok(())
    }

    pub fn save_config(&self) {
        debug!("Saving configuration to {:?}", self.config_path);
        let file: File = File::create(self.config_path.clone())
            .map_err(|e| AppError::Internal {
                message: format!("Failed to create config file {:?}: {}", self.config_path, e),
            })
            .unwrap();

        let obj = FileConfigInfo {
            api_port: Some(self.api_port),
            proxy_port: Some(self.proxy_port),
            port_min: Some(self.port_range.0),
            port_max: Some(self.port_range.1),
            mount_point: Some(self.mount_point.clone()),
            default_project: self.default_project.clone(),
            postgres_config: Some(PostgresConfig {
                user: self.postgres_config.user.clone(),
                password: self.postgres_config.password.clone(),
                database: self.postgres_config.database.clone(),
            }),
            projects: Some(self.projects.clone()),
        };

        let mut writer = BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, &obj)
            .map_err(|e| AppError::Internal {
                message: format!("Failed to write config file {:?}: {}", self.path, e),
            })
            .unwrap();
        debug!("Configuration saved successfully");
    }
}
