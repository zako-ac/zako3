use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZakoError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type ZakoResult<T> = Result<T, ZakoError>;
