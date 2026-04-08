use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Database error: {0}")]
    DbError(#[from] sqlx::Error),
    #[error("Database error: {0}")]
    DbMigrationError(#[from] sqlx::migrate::MigrateError),
    #[error("Configuration error: {0}")]
    ConfigError(#[from] config::ConfigError),
    #[error("Environment variable error: {0}")]
    EnvError(#[from] std::env::VarError),
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Serde JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("JWT error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),
    #[error("Entity not found: {0}")]
    NotFound(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Internal server error: {0}")]
    Internal(String),
    #[error("State service error: {0}")]
    StateError(#[from] zako3_states::StateServiceError),
}

pub type CoreResult<T> = Result<T, CoreError>;
