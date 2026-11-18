use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Structured error from the TapHub subsystem. Serializable so it can cross
/// the taphub-transport and ae-protocol wire boundaries while preserving
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

/// Why a session turned out to have no consumer: nothing that would ever pull
/// the mixed audio out of it, so anything queued there is played to nobody.
///
/// This is the failure the audio engine used to swallow: a session was
/// registered, `play` answered `Ok`, and the channel went permanently silent
/// while every message looked like it had been read. The reason is carried as a
/// stable code so the engine's log line and HQ's `dropped_reason` field name the
/// same cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoConsumerReason {
    /// Songbird holds no `Call` for the guild at all: nothing ever joined.
    ManagerMissing,
    /// The `Call` lock could not be taken within the timeout, so the play could
    /// not be handed to the driver.
    CallLockTimeout,
    /// The `Call` exists but has no live voice connection, so there is no driver
    /// to play to.
    NoConnection,
    /// The session's mixer thread has exited; its ring buffer has no reader, so
    /// a source added to it would never reach the driver.
    MixerDead,
}

impl NoConsumerReason {
    /// The stable code, as it appears in logs and in HQ's `dropped_reason`.
    pub const fn as_str(self) -> &'static str {
        match self {
            NoConsumerReason::ManagerMissing => "manager_missing",
            NoConsumerReason::CallLockTimeout => "call_lock_timeout",
            NoConsumerReason::NoConnection => "no_connection",
            NoConsumerReason::MixerDead => "mixer_dead",
        }
    }
}

impl std::fmt::Display for NoConsumerReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
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

    /// No consumer is attached, so this audio could never be played. Returned
    /// instead of a silent success: the caller has to hear that the request was
    /// not played, not learn it from a channel that has gone quiet.
    #[error("no audio consumer: {0}")]
    NoConsumer(NoConsumerReason),

    #[error("RPC error: {0}")]
    Rpc(String),
}

pub type ZakoResult<T> = Result<T, ZakoError>;
