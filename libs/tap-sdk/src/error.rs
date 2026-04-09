use zakofish::error::ZakofishError;
use zakofish::types::message::AudioRequestFailureMessage;

#[derive(Debug, thiserror::Error)]
pub enum TapError {
    /// Transient failure. The Hub will try another tap for this request.
    /// Use for: network errors, rate limits, timeouts, yt-dlp crashes.
    #[error("{0}")]
    Retriable(String),

    /// Permanent failure. The Hub will not retry on another tap.
    /// Use for: unsupported URL scheme, video unavailable, age-restricted content.
    #[error("{0}")]
    Permanent(String),
}

impl TapError {
    pub(crate) fn into_wire(self) -> AudioRequestFailureMessage {
        match self {
            TapError::Retriable(reason) => AudioRequestFailureMessage {
                reason,
                try_others: true,
            },
            TapError::Permanent(reason) => AudioRequestFailureMessage {
                reason,
                try_others: false,
            },
        }
    }
}

/// Top-level SDK error (returned from TapBuilder::run).
#[derive(Debug, thiserror::Error)]
pub enum SdkError {
    #[error("hub rejected connection: {0}")]
    Rejected(String),

    #[error("connection error: {0}")]
    Connection(#[from] ZakofishError),

    #[error("tls config error: {0}")]
    Tls(String),

    #[error("invalid hub address: {0}")]
    AddrParse(#[from] std::net::AddrParseError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
