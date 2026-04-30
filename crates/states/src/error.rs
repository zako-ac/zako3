use thiserror::Error;

#[derive(Error, Debug)]
pub enum StateServiceError {
    #[error("Cache error")]
    CacheError,
    #[cfg(feature = "redis")]
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),
}

pub type Result<T> = std::result::Result<T, StateServiceError>;
