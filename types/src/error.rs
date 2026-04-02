use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZakoError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Symphonia error: {0}")]
    Symphonia(#[from] symphonia::core::errors::Error),

    #[error("Decoding error: {0}")]
    Decoding(String),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Serde JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Songbird error: {0}")]
    Songbird(#[from] songbird::error::JoinError),

    #[error("Crossbeam recv error: {0}")]
    CrossbeamRecv(#[from] crossbeam::channel::RecvError),
}

pub type ZakoResult<T> = Result<T, ZakoError>;
