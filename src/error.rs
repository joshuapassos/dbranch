use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Internal server error: {message}")]
    Internal { message: String },

    // Configuration errors
    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("Failed to parse configuration file: {message}")]
    ConfigParsing { message: String },

    // File system errors
    #[error("File operation failed: {message}")]
    FileSystem { message: String },

    #[error("File not found: {path}")]
    FileNotFound { path: String },

    // Project and database errors
    #[error("Project '{name}' already exists")]
    ProjectAlreadyExists { name: String },

    #[error("Project '{name}' not found")]
    ProjectNotFound { name: String },

    #[error("Default Project not found")]
    DefaultProjectNotFound,

    #[error("Database operation failed: {message}")]
    Database { message: String },

    // Network and port errors
    #[error("No available ports found in range {min}-{max}")]
    NoPortAvailable { min: u16, max: u16 },

    #[error("Network operation failed: {message}")]
    Network { message: String },

    // Authentication and permissions
    #[error("Authentication failed: {message}")]
    Auth { message: String },

    #[error("Permission denied: {message}")]
    Permission { message: String },

    // BTRFS and disk operations
    #[error("BTRFS operation failed: {message}")]
    Btrfs { message: String },

    #[error("Disk mount operation failed: {message}")]
    DiskMount { message: String },

    // Docker operations
    #[error("Docker operation failed: {message}")]
    Docker { message: String },

    // Command not implemented
    #[error("Command '{command}' is not implemented")]
    NotImplemented { command: String },
}
