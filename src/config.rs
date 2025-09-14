use std::{
    fs::{self, File},
    io::BufWriter,
    net::TcpListener,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::{
    cli::{Branch, Project},
    error::AppError,
};

#[derive(Clone)]
pub struct FolderConfig {
    path: String,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Approach {
    NewDisk,
    ExistingDisk,
}

impl<'de> Deserialize<'de> for Approach {
    fn deserialize<D>(deserializer: D) -> Result<Approach, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "NEW_DISK" => Ok(Approach::NewDisk),
            "EXISTING_DISK" => Ok(Approach::ExistingDisk),
            _ => Err(serde::de::Error::unknown_variant(
                &s,
                &["NEW_DISK", "EXISTING_DISK"],
            )),
        }
    }
}

impl Serialize for Approach {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            Approach::NewDisk => "NEW_DISK",
            Approach::ExistingDisk => "EXISTING_DISK",
        };
        serializer.serialize_str(s)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Eq, Clone)]
pub struct FileConfigInfo {
    pub api_port: Option<u16>,
    pub proxy_port: Option<u16>,
    pub approach: Option<Approach>,
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
    pub approach: Approach,
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
    pub fn from_env() -> Self {
        let config_path = std::env::var("DBRANCH_CONFIG").unwrap_or(".config".to_string());
        debug!("Loading folder config from path: {}", config_path);
        Self {
            path: config_path.parse().unwrap(),
        }
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
            approach: c
                .approach
                .or_else(|| {
                    std::env::var("DBRANCH_APPROACH")
                        .ok()
                        .and_then(|v| match v.as_str() {
                            "EXISTING_DISK" => Some(Approach::ExistingDisk),
                            _ => Some(Approach::NewDisk),
                        })
                })
                .unwrap_or(Approach::NewDisk),
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
                password: "dbranch_pass".into(),
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
        let folder_config = FolderConfig::from_env();

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
            AppError::ConfigParsing {
                message: format!("Failed to parse config file {}", e),
            }
        });

        let parsed_config = json
            .and_then(|op| Config::from_env(Path::new(&folder_config.path), op))
            .map_err(|e| AppError::Config {
                message: format!("Failed to read config file: {}", e),
            });

        if needs_create {
            info!("Creating new config file at {:?}", file_config);
            let file = File::create(file_config.clone()).map_err(|e| AppError::FileSystem {
                message: format!(
                    "Failed to create config file {:?}: {}",
                    file_config.as_os_str(),
                    e
                ),
            })?;

            let c = parsed_config.as_ref().unwrap();

            let obj = FileConfigInfo {
                api_port: Some(c.api_port),
                proxy_port: Some(c.proxy_port),
                approach: Some(c.approach.clone()),
                port_min: Some((c.port_range).0),
                port_max: Some((c.port_range).1),
                mount_point: Some(c.mount_point.clone()),
                default_project: c.default_project.clone(),
                postgres_config: Some(PostgresConfig {
                    user: c.postgres_config.user.clone(),
                    password: c.postgres_config.password.clone(),
                    database: c.postgres_config.database.clone(),
                }),
                projects: Some(vec![]),
            };

            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, &obj).map_err(|e| AppError::FileSystem {
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
        if !self.projects.iter().any(|p| p == &project) {
            debug!("Project {} not found in project list", project);
            return Err(AppError::ProjectNotFound { name: project });
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

    pub fn remove_project(&mut self, name: &str) -> Result<(), AppError> {
        info!("Removing project: {}", name);

        if !self.projects.contains(&name.to_string()) {
            debug!("Project {} not found in project list", name);
            return Err(AppError::ProjectNotFound {
                name: name.to_string(),
            });
        }

        // Remove from projects list
        self.projects.retain(|p| p != name);
        debug!("Project {} removed from config list", name);

        // Remove project directory
        let project_dir = self.path.join(name);
        debug!("Removing project directory: {:?}", project_dir);

        if project_dir.exists() {
            match fs::remove_dir_all(&project_dir) {
                Ok(_) => {
                    info!("Project directory removed successfully");
                }
                Err(e) => {
                    debug!("Failed to remove project directory: {}", e);
                    return Err(AppError::FileSystem {
                        message: format!("Failed to remove project directory: {}", e),
                    });
                }
            }
        }

        debug!("Total projects now: {}", self.projects.len());
        self.save_config();
        info!("Project {} removed successfully", name);
        Ok(())
    }

    pub fn create_branch(
        &self,
        project_name: String,
        branch_name: String,
        valid_port: u16,
    ) -> Result<(), AppError> {
        let project_path = self.path.join(project_name);
        let metadata_path = project_path.join("metadata.json");

        debug!("Looking for metadata file at: {:?}", metadata_path);

        let metadata_content = fs::read_to_string(&metadata_path).ok().unwrap();
        let mut project: Project = serde_json::from_str(&metadata_content).ok().unwrap();

        project.branches.push(Branch {
            name: branch_name,
            port: valid_port,
            created_at: Utc::now(),
        });

        return self.save_project_changes(&project);
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

        let metadata = File::create(metadata_path).map_err(|e| AppError::FileSystem {
            message: format!("Failed to create metadata file: {}", e),
        })?;

        let mut writer = BufWriter::new(metadata);
        serde_json::to_writer_pretty(&mut writer, &project).map_err(|e| AppError::FileSystem {
            message: format!("Failed to write metadata file: {}", e),
        })?;
        debug!("Metadata file written successfully");

        Ok(())
    }

    fn save_project_changes(&self, project: &Project) -> Result<(), AppError> {
        let metadata_path = Path::new(&self.path)
            .join(project.name.clone())
            .join("metadata.json");
        debug!("Saving changes to metadata file: {:?}", metadata_path);

        let metadata = File::create(metadata_path).map_err(|e| AppError::FileSystem {
            message: format!("Failed to create metadata file: {}", e),
        })?;

        let mut writer = BufWriter::new(metadata);
        serde_json::to_writer_pretty(&mut writer, &project).map_err(|e| AppError::FileSystem {
            message: format!("Failed to write metadata file: {}", e),
        })?;
        debug!("Metadata file updated successfully");

        Ok(())
    }

    pub fn set_active_branch(
        &self,
        project_name: String,
        branch_name: String,
    ) -> Result<(), AppError> {
        let project_path = self.path.join(project_name);
        let metadata_path = project_path.join("metadata.json");

        debug!("Looking for metadata file at: {:?}", metadata_path);

        let metadata_content = fs::read_to_string(&metadata_path).ok().unwrap();
        let mut project: Project = serde_json::from_str(&metadata_content).ok().unwrap();

        if project.branches.iter().any(|b| b.name == branch_name) || branch_name == "main" {
            project.active_branch = if branch_name == "main" {
                None
            } else {
                Some(branch_name)
            };
            return self.save_project_changes(&project);
        } else {
            Err(AppError::BranchNotFound { name: branch_name })
        }
    }

    pub fn save_config(&self) {
        debug!("Saving configuration to {:?}", self.config_path);
        let file: File = File::create(self.config_path.clone())
            .map_err(|e| AppError::FileSystem {
                message: format!("Failed to create config file {:?}: {}", self.config_path, e),
            })
            .unwrap();

        let obj = FileConfigInfo {
            api_port: Some(self.api_port),
            approach: Some(self.approach.clone()),
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
            .map_err(|e| AppError::FileSystem {
                message: format!("Failed to write config file {:?}: {}", self.path, e),
            })
            .unwrap();
        debug!("Configuration saved successfully");
    }
}
