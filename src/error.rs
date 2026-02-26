#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("project not found: {0}")]
    ProjectNotFound(String),

    #[error("environment not found: {0}")]
    EnvironmentNotFound(String),

    #[error("config item not found: {0}")]
    ConfigItemNotFound(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("storage error: {0}")]
    StorageError(String),

    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ConfigError>;
