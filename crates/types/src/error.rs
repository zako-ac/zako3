use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Structured error from the TapHub subsystem. Serializable so it can cross
/// the taphub-transport and tl-protocol wire boundaries while preserving
/// enough type information for the bot to render a localized user message.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum TapHubError {
    #[error("Tap unavailable and no cached metadata found")]
    TapUnavailable,
    #[error("Tap metadata not found: {0}")]
    TapNotFound(String),
    #[error("Access denied for tap {0}")]
    PermissionDenied(String),
    /// Tap-script-authored failure surfaced via `AudioRequestFailureMessage`.
    /// `try_others` mirrors `tap_sdk::TapError::{Retriable,Permanent}`.
    #[error("Tap script error: {reason}")]
    TapScript { reason: String, try_others: bool },
    /// Infrastructure failure not otherwise categorized (transport, decode, etc).
    #[error("TapHub internal error: {0}")]
    Internal(String),
}

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

    #[error(transparent)]
    TapHub(#[from] TapHubError),

    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("taphub request timed out")]
    TaphubTimeout,
}

pub type ZakoResult<T> = Result<T, ZakoError>;
