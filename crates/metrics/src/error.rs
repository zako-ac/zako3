use thiserror::Error;

#[derive(Error, Debug)]
pub enum MetricsError {
    #[error("Redis error: {0}")]
    Redis(#[from] zako3_states::StateServiceError),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, MetricsError>;
