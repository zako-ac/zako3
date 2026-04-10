use thiserror::Error;

#[derive(Debug, Error)]
pub enum TlError {
    #[error("No AE connected for the given route")]
    NoSuchAe,

    #[error("Transport error: {0}")]
    Transport(String),
}
