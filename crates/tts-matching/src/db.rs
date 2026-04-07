use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::Result;

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

const SCHEMA: &str = "
    PRAGMA journal_mode = WAL;

    CREATE TABLE IF NOT EXISTS wasm_mappers (
        id            TEXT    PRIMARY KEY NOT NULL,
        name          TEXT    NOT NULL,
        wasm_filename TEXT    NOT NULL,
        sha256_hash   TEXT    NOT NULL,
        input_data    TEXT    NOT NULL DEFAULT '[]',
        created_at    INTEGER NOT NULL,
        updated_at    INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS pipeline_order (
        position  INTEGER PRIMARY KEY,
        mapper_id TEXT    NOT NULL
    );
";

impl Db {
    pub async fn open(path: PathBuf) -> Result<Self> {
        tokio::task::spawn_blocking(move || -> Result<Self> {
            let conn = rusqlite::Connection::open(&path)?;
            conn.execute_batch(SCHEMA)?;
            Ok(Self {
                conn: Arc::new(Mutex::new(conn)),
            })
        })
        .await?
    }

    /// Helper to get a lock on the connection for a sync operation.
    pub(crate) fn conn(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.conn
            .lock()
            .expect("connection mutex poisoned")
    }
}
