use std::io;

#[derive(Debug, thiserror::Error)]
pub enum TlError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Handshake failed: {0}")]
    Handshake(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Response ID mismatch: expected {expected}, got {got}")]
    ResponseIdMismatch { expected: u64, got: u64 },
}
