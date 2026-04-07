use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("wasm error: {0}")]
    Wasm(#[from] wasmtime::Error),

    #[error("wasm file not found: {path}")]
    WasmNotFound { path: PathBuf },

    #[error("sha256 mismatch for {mapper_id}: expected {expected}, got {actual}")]
    HashMismatch {
        mapper_id: String,
        expected: String,
        actual: String,
    },

    #[error("mapper not found: {0}")]
    NotFound(String),

    #[error("task join error")]
    Join(#[from] tokio::task::JoinError),
}

pub type Result<T> = std::result::Result<T, Error>;
