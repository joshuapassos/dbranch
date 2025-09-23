use std::path::Path;

use docker_wrapper::{
    DockerCommand, InspectCommand, NetworkCreateCommand, NetworkLsCommand, RmCommand, RunCommand,
    StopCommand,
};
use tracing::{debug, info};

use crate::{
    config::{Branch, Config},
    error::AppError,
};

pub trait DatabaseOperator {
    async fn create_database(&self, config: Config, port: u16, name: &str) -> Result<(), AppError>;
    async fn delete_database(&self, config: Config, name: &str) -> Result<(), AppError>;
    async fn stop_database(&self, config: Config, name: &str) -> Result<(), AppError>;
    async fn list_databases(&self, config: Config) -> Result<Vec<Branch>, AppError>;
    async fn get_database_info(&self, config: Config, name: &str) -> Result<Branch, AppError>;
    async fn is_container_running(&self, name: &str) -> Result<bool, AppError>;
}

pub struct PostgresOperator {}

impl PostgresOperator {
    pub fn new() -> Self {
        debug!("Creating new PostgresOperator instance");
        Self {}
    }
}

impl DatabaseOperator for PostgresOperator {
    async fn create_database(&self, config: Config, port: u16, name: &str) -> Result<(), AppError> {
        info!(
            "Creating PostgreSQL database '{}' for project '{}' on port {}",
            name, config.name, port
        );

        debug!("Creating Docker network 'dbranch-network'");

        let net = NetworkLsCommand::new()
            .filter("name", "dbranch-network")
            .execute()
            .await
            .map_err(|e| AppError::Docker {
                message: format!("Failed to list Docker networks: {}", e),
            })?;

        if net.success && net.stdout.contains("dbranch-network") {
            debug!("Docker network 'dbranch-network' already exists");
        } else {
            debug!("Docker network 'dbranch-network' does not exist, creating it");
            let _ = NetworkCreateCommand::new("dbranch-network")
                .execute()
                .await
                .map_err(|e| AppError::Docker {
                    message: format!("Failed to create Docker network: {}", e),
                })?;
            debug!("Docker network created successfully");
        }

        let volume_path = Path::new(config.mount_point.clone().as_str())
            .join(&config.name)
            .join(&name)
            .join("data")
            .to_string_lossy()
            .into_owned();

        std::fs::create_dir_all(volume_path.clone()).unwrap();
        // https://github.com/docker-library/docs/tree/master/postgres#arbitrary---user-notes
        std::os::unix::fs::chown(volume_path.clone(), Some(1000), Some(1000)).unwrap();

        debug!(
            "Setting up PostgreSQL container with volume: {}",
            volume_path
        );
        debug!(
            "Container configuration: user={}, database={}",
            config.postgres_config.clone().unwrap().user,
            config
                .postgres_config
                .clone()
                .unwrap()
                .database
                .as_ref()
                .unwrap_or(&"dbranch".to_string())
        );

        let _output = RunCommand::new("postgres:17-alpine")
            .name(format!("{}_{}", config.name, name))
            .port(port, 5432)
            .network("dbranch-network")
            .user("1000:1000") // This allow the container to run with the host user permissions
            .volume(volume_path, "/var/lib/postgresql/data")
            .env(
                "POSTGRES_USER",
                config.postgres_config.clone().unwrap().user.as_str(),
            )
            .env(
                "POSTGRES_PASSWORD",
                config.postgres_config.clone().unwrap().password.as_str(),
            )
            .env(
                "POSTGRES_DB",
                config
                    .postgres_config
                    .clone()
                    .unwrap()
                    .database
                    .clone()
                    .or(Some("dbranch".into()))
                    .unwrap(),
            )
            .env("PGDATA", "/var/lib/postgresql/data/pgdata")
            .restart("no")
            .detach()
            .execute()
            .await
            .unwrap();

        info!(
            "PostgreSQL container '{}' created successfully on port {}",
            name, port
        );

        Ok(())
    }

    async fn delete_database(&self, config: Config, name: &str) -> Result<(), AppError> {
        info!(
            "Deleting PostgreSQL database '{}' for project '{}'",
            name, config.name
        );

        debug!("Stopping and removing PostgreSQL container: {}", name);

        let stop_output = StopCommand::new(format!("{}_{}", config.name, name))
            .execute()
            .await
            .map_err(|e| AppError::Docker {
                message: format!(
                    "Failed to stop Docker container {}: {}",
                    format!("{}_{}", config.name, name),
                    e
                ),
            })?;

        if !(stop_output.is_success()) {
            debug!(
                "Container {} might already be stopped: {}",
                name, stop_output.stderr
            );
        } else {
            info!("Container {} stopped successfully", name);
        }

        let rm_output = RmCommand::new(format!("{}_{}", config.name, name))
            .volumes()
            .execute()
            .await
            .map_err(|e| AppError::Docker {
                message: format!(
                    "Failed to remove Docker container {}: {}",
                    format!("{}_{}", config.name, name),
                    e
                ),
            })?;

        if !(rm_output.removed_contexts().len() > 0) {
            debug!(
                "Container {} might already be stopped: {}",
                name, rm_output.stderr
            );
        } else {
            info!("Container {} stopped successfully", name);
        }

        info!("PostgreSQL container '{}' deleted successfully", name);
        Ok(())
    }

    async fn stop_database(&self, config: Config, name: &str) -> Result<(), AppError> {
        let container_name = format!("{}_{}", config.name, name);

        info!(
            "Stopping PostgreSQL database '{}' for project '{}'",
            container_name, config.name
        );

        debug!("Stopping PostgreSQL container: {}", container_name);

        let stop_output = StopCommand::new(container_name.clone())
            .execute()
            .await
            .map_err(|e| AppError::Docker {
                message: format!("Failed to stop Docker container {}: {}", container_name, e),
            })?;

        if !stop_output.is_success() {
            debug!(
                "Container {} might already be stopped: {}",
                container_name, stop_output.stderr
            );
        } else {
            info!("Container {} stopped successfully", container_name);
        }

        info!(
            "PostgreSQL container '{}' stopped successfully",
            container_name
        );
        Ok(())
    }

    async fn list_databases(&self, config: Config) -> Result<Vec<Branch>, AppError> {
        debug!("Listing PostgreSQL databases for project '{}'", config.name);
        // TODO: Implement logic to list PostgreSQL databases here
        Ok(vec![])
    }

    async fn get_database_info(&self, config: Config, name: &str) -> Result<Branch, AppError> {
        debug!(
            "Getting database info for '{}' in project '{}'",
            name, config.name
        );
        // TODO: Implement logic to get information about a specific PostgreSQL database here
        Err(AppError::NotImplemented {
            command: "get_database_info".into(),
        })
    }

    async fn is_container_running(&self, name: &str) -> Result<bool, AppError> {
        debug!("Checking if container '{}' is running", name);

        let inspect_output = InspectCommand::new(name).execute().await;

        match inspect_output {
            Ok(output) => {
                if output.success && !output.stdout.is_empty() {
                    let is_running = output.stdout.contains("\"Running\":true")
                        || output.stdout.contains("\"Running\": true");
                    debug!("Container '{}' running status: {}", name, is_running);
                    Ok(is_running)
                } else {
                    debug!("Container '{}' not found or inspect failed", name);
                    Ok(false)
                }
            }
            Err(e) => {
                debug!("Failed to inspect container '{}': {}", name, e);
                Ok(false)
            }
        }
    }
}
