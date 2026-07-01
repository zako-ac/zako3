use std::fmt::Debug;

#[derive(Debug, thiserror::Error)]
pub enum ZakofishError {
    #[error("Protofish3 error: {0}")]
    Protofish3Error(#[from] protofish3::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] rmp_serde::encode::Error),
    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] rmp_serde::decode::Error),
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    /// Tap-script-authored failure (from `AudioRequestFailureMessage`).
    /// `try_others` mirrors `tap_sdk::TapError::{Retriable,Permanent}`.
    #[error("Tap request failed: {reason}")]
    TapRequestFailure { reason: String, try_others: bool },
}

pub type Result<T> = std::result::Result<T, ZakofishError>;
