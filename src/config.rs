use std::{
    fs::{self, File},
    io::BufWriter,
    net::TcpListener,
    path::Path,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::AppError;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Eq)]
pub struct Branch {
    pub name: String,
    pub port: u16,
    pub is_main: bool,
    pub created_at: DateTime<Utc>,
}

pub static DEFAULT_CONFIG_PATH: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    std::env::var("DBRANCH_CONFIG").unwrap_or(String::from(".dbranch.config.json"))
});

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
pub struct Config {
    pub name: String,
    pub api_port: u16,
    pub proxy_port: u16,
    pub created_at: DateTime<Utc>,
    pub approach: Approach,
    pub port_min: u16,
    pub port_max: u16,
    pub mount_point: String,
    pub active_branch: Option<String>,
    pub postgres_config: Option<PostgresConfig>,
    pub branches: Vec<Branch>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PostgresConfig {
    pub user: String,
    pub password: String,
    pub database: Option<String>,
}

impl Config {
    pub fn new(name: String) -> Self {
        Config {
            name: name,
            api_port: 8000,
            proxy_port: 5432,
            approach: Approach::ExistingDisk,
            port_min: 7000,
            port_max: 7999,
            mount_point: String::from("/mnt/dbranch"),
            active_branch: None,
            created_at: Utc::now(),
            postgres_config: Some(PostgresConfig {
                user: String::from("dbranch_user"),
                password: String::from("dbranch_password"),
                database: None,
            }),
            branches: vec![Branch {
                name: String::from("main"),
                port: get_valid_port(7000, 7999).unwrap_or(7000),
                is_main: true,
                created_at: Utc::now(),
            }],
        }
    }

    pub fn from_file() -> Result<Self, AppError> {
        debug!("Loading configuration from file");
        let binding = std::env::var("DBRANCH_CONFIG").unwrap_or(".dbranch.config.json".to_string());
        let file_config = Path::new(&binding);

        debug!("Config file path: {:?}", file_config);

        match fs::read_to_string(file_config) {
            Ok(content) => {
                debug!("Config file exists, reading content");
                let json = serde_json::from_str::<Config>(content.as_str()).map_err(|e| {
                    AppError::ConfigParsing {
                        message: format!("Failed to parse config file {}", e),
                    }
                });

                return json.map_err(|e| AppError::Config {
                    message: format!("Failed to read config file: {}", e),
                });
            }
            Err(_) => {
                debug!("Config file doesn't exist, will create with defaults");
                let parsed_config = Config::new("my_project".to_string());
                parsed_config.save_config();
                return Ok(parsed_config);
            }
        };
    }

    pub fn get_valid_port(&self) -> Option<u16> {
        get_valid_port(self.port_min, self.port_max)
    }

    pub fn create_branch(&mut self, branch_name: String, valid_port: u16) {
        self.branches.push(Branch {
            name: branch_name,
            port: valid_port,
            is_main: false,
            created_at: Utc::now(),
        });

        self.save_config();
    }

    pub fn set_active_branch(&mut self, branch_name: String) -> Result<(), AppError> {
        if self.branches.iter().any(|b| b.name == branch_name) || branch_name == "main" {
            self.active_branch = if branch_name == "main" {
                None
            } else {
                Some(branch_name)
            };
            self.save_config();
            return Ok(());
        } else {
            Err(AppError::BranchNotFound { name: branch_name })
        }
    }

    pub fn save_config(&self) {
        debug!("Saving configuration to {:?}", DEFAULT_CONFIG_PATH);
        let file: File = File::create(DEFAULT_CONFIG_PATH.as_str())
            .map_err(|e| AppError::FileSystem {
                message: format!(
                    "Failed to create config file {:?}: {}",
                    DEFAULT_CONFIG_PATH, e
                ),
            })
            .unwrap();

        let mut writer = BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, &self)
            .map_err(|e| AppError::FileSystem {
                message: format!(
                    "Failed to write config file {:?}: {}",
                    DEFAULT_CONFIG_PATH, e
                ),
            })
            .unwrap();
        debug!("Configuration saved successfully");
    }
}

pub fn get_valid_port(port_min: u16, port_max: u16) -> Option<u16> {
    debug!(
        "Searching for available port in range {}-{}",
        port_min, port_max
    );
    for port in port_min..=port_max {
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
        port_min, port_max
    );
    None
}
