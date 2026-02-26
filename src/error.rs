#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("project not found: {0}")]
    ProjectNotFound(String),

    #[error("project already exists: {0}")]
    ProjectAlreadyExists(String),

    #[error("environment not found: {0}")]
    EnvironmentNotFound(String),

    #[error("environment already exists: {0}")]
    EnvironmentAlreadyExists(String),

    #[error("config item not found: {0}")]
    ConfigItemNotFound(String),

    #[error("config item already exists: {0}")]
    ConfigItemAlreadyExists(String),

    #[error("api key not found: {0}")]
    ApiKeyNotFound(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("storage error: {0}")]
    StorageError(String),

    #[error("serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ConfigError>;
