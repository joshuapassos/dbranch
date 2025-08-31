use std::path::Path;

use docker_wrapper::{DockerCommand, NetworkCreateCommand, RunCommand};
use tracing::{debug, info};

use crate::{
    cli::{Branch, Project},
    config::Config,
    error::AppError,
};

pub trait DatabaseOperator {
    async fn create_database(
        &self,
        project: Project,
        config: Config,
        port: u16,
        name: &str,
    ) -> Result<(), AppError>;
    async fn delete_database(&self, project: Project, name: &str) -> Result<(), AppError>;
    async fn list_databases(&self, project: Project) -> Result<Vec<Branch>, AppError>;
    async fn get_database_info(&self, project: Project, name: &str) -> Result<Branch, AppError>;
}

pub struct PostgresOperator {}

impl PostgresOperator {
    pub fn new() -> Self {
        debug!("Creating new PostgresOperator instance");
        Self {}
    }
}

impl DatabaseOperator for PostgresOperator {
    async fn create_database(
        &self,
        project: Project,
        config: Config,
        port: u16,
        name: &str,
    ) -> Result<(), AppError> {
        info!("Creating PostgreSQL database '{}' for project '{}' on port {}", name, project.name, port);
        
        debug!("Creating Docker network 'dbranch-network'");
        let _ = NetworkCreateCommand::new("dbranch-network")
            .execute()
            .await
            .unwrap();
        debug!("Docker network created successfully");

        let volume_path = Path::new(config.mount_point.clone().as_str())
            .join(&name)
            .to_string_lossy()
            .into_owned();
        
        debug!("Setting up PostgreSQL container with volume: {}", volume_path);
        debug!("Container configuration: user={}, database={}", 
               config.postgres_config.user,
               config.postgres_config.database.as_ref().unwrap_or(&"dbranch".to_string()));
        
        let output = RunCommand::new("postgres:17-alpine")
            .name(name)
            .port(port, 5432)
            .network("dbranch-network")
            .volume(
                volume_path,
                "/var/lib/postgresql/data",
            )
            .env("POSTGRES_USER", config.postgres_config.user.as_str())
            .env(
                "POSTGRES_PASSWORD",
                config.postgres_config.password.as_str(),
            )
            .env(
                "POSTGRES_DB",
                config
                    .postgres_config
                    .database
                    .or(Some("dbranch".into()))
                    .unwrap(),
            )
            .env("PGDATA", "/var/lib/postgresql/data/pgdata")
            .restart("unless-stopped")
            .health_cmd(format!(
                "pg_isready -U {}",
                config.postgres_config.user.as_str()
            ))
            .health_interval("10s")
            .health_timeout("5s")
            .health_retries(5)
            .detach()
            .execute()
            .await
            .unwrap();
        
        info!("PostgreSQL container '{}' created successfully on port {}", name, port);

        Ok(())
    }

    async fn delete_database(&self, project: Project, name: &str) -> Result<(), AppError> {
        info!("Deleting PostgreSQL database '{}' for project '{}'", name, project.name);
        debug!("Delete operation not yet implemented");
        // TODO: Implement PostgreSQL database deletion logic here
        Ok(())
    }

    async fn list_databases(&self, project: Project) -> Result<Vec<Branch>, AppError> {
        debug!("Listing PostgreSQL databases for project '{}'", project.name);
        // TODO: Implement logic to list PostgreSQL databases here
        Ok(vec![])
    }

    async fn get_database_info(&self, project: Project, name: &str) -> Result<Branch, AppError> {
        debug!("Getting database info for '{}' in project '{}'", name, project.name);
        // TODO: Implement logic to get information about a specific PostgreSQL database here
        Err(AppError::Internal {
            message: "Not implemented".into(),
        })
    }
}
