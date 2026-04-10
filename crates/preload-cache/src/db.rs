use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use tokio::io;

// ---------------------------------------------------------------------------
// DbEntry — mirrors the cache_entries table
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DbEntry {
    pub tap_id: String,
    /// serde_json of `AudioCacheItemKey`
    pub cache_key: String,
    /// Absolute path to the `.opus` file; `None` for metadata-only entries.
    pub opus_path: Option<String>,
    /// Absolute path to the `.json` sidecar file (always present).
    pub json_path: String,
    /// Unix seconds UTC; `None` means no expiry.
    pub expire_at: Option<i64>,
    pub use_count: i64,
    /// Unix seconds UTC.
    pub last_used_at: Option<i64>,
    /// Unix seconds UTC.
    pub created_at: i64,
    pub gdsf_priority: f64,
    pub is_downloading: bool,
}

// ---------------------------------------------------------------------------
// CacheDb
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct CacheDb {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS cache_entries (
        id             INTEGER PRIMARY KEY AUTOINCREMENT,
        tap_id         TEXT    NOT NULL,
        cache_key      TEXT    NOT NULL,
        opus_path      TEXT,
        json_path      TEXT    NOT NULL DEFAULT '',
        expire_at      INTEGER,
        use_count      INTEGER NOT NULL DEFAULT 0,
        last_used_at   INTEGER,
        created_at     INTEGER NOT NULL,
        gdsf_priority  REAL    NOT NULL DEFAULT 0.0,
        is_downloading INTEGER NOT NULL DEFAULT 0,
        UNIQUE (tap_id, cache_key)
    );
";

impl CacheDb {
    pub async fn open(path: PathBuf) -> io::Result<Self> {
        tokio::task::spawn_blocking(move || -> io::Result<Self> {
            Self::open_inner(&path)
        })
        .await
        .map_err(io::Error::other)?
    }

    fn open_inner(path: &PathBuf) -> io::Result<Self> {
        match Self::try_open(path) {
            Ok(db) => Ok(db),
            Err(e) => {
                if is_corrupt_error(&e) {
                    tracing::warn!(path = %path.display(), "cache.db is corrupt, deleting and recreating");
                    let _ = std::fs::remove_file(path);
                    Self::try_open(path)
                } else {
                    Err(e)
                }
            }
        }
    }

    fn try_open(path: &PathBuf) -> io::Result<Self> {
        let conn = rusqlite::Connection::open(path)
            .map_err(|e| rusqlite_to_io(e, "open"))?;
        conn.execute_batch(&format!("PRAGMA journal_mode = WAL;\n{}", SCHEMA))
            .map_err(|e| rusqlite_to_io(e, "schema"))?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// Insert or replace a cache entry (sets `is_downloading = 1`).
    pub async fn insert(&self, entry: DbEntry) -> io::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<()> {
            let conn = conn.lock().map_err(|_| io::Error::other("lock poisoned"))?;
            conn.execute(
                "INSERT OR REPLACE INTO cache_entries
                    (tap_id, cache_key, opus_path, json_path, expire_at, use_count, last_used_at,
                     created_at, gdsf_priority, is_downloading)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    entry.tap_id,
                    entry.cache_key,
                    entry.opus_path,
                    entry.json_path,
                    entry.expire_at,
                    entry.use_count,
                    entry.last_used_at,
                    entry.created_at,
                    entry.gdsf_priority,
                    entry.is_downloading as i64,
                ],
            )
            .map_err(io::Error::other)?;
            Ok(())
        })
        .await
        .map_err(io::Error::other)?
    }

    /// Mark an entry as fully written (`is_downloading = 0`).
    pub async fn mark_complete(&self, tap_id: String, cache_key: String) -> io::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<()> {
            let conn = conn.lock().map_err(|_| io::Error::other("lock poisoned"))?;
            conn.execute(
                "UPDATE cache_entries SET is_downloading = 0 WHERE tap_id = ?1 AND cache_key = ?2",
                rusqlite::params![tap_id, cache_key],
            )
            .map_err(io::Error::other)?;
            Ok(())
        })
        .await
        .map_err(io::Error::other)?
    }

    /// Look up an entry by `(tap_id, cache_key)`.
    pub async fn get(&self, tap_id: String, cache_key: String) -> io::Result<Option<DbEntry>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<Option<DbEntry>> {
            let conn = conn.lock().map_err(|_| io::Error::other("lock poisoned"))?;
            let mut stmt = conn
                .prepare(
                    "SELECT tap_id, cache_key, opus_path, json_path, expire_at, use_count, last_used_at,
                            created_at, gdsf_priority, is_downloading
                     FROM cache_entries WHERE tap_id = ?1 AND cache_key = ?2",
                )
                .map_err(io::Error::other)?;

            let mut rows = stmt
                .query(rusqlite::params![tap_id, cache_key])
                .map_err(io::Error::other)?;

            if let Some(row) = rows.next().map_err(io::Error::other)? {
                Ok(Some(row_to_entry(row)?))
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(io::Error::other)?
    }

    /// Delete an entry by `(tap_id, cache_key)`.
    pub async fn delete(&self, tap_id: String, cache_key: String) -> io::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<()> {
            let conn = conn.lock().map_err(|_| io::Error::other("lock poisoned"))?;
            conn.execute(
                "DELETE FROM cache_entries WHERE tap_id = ?1 AND cache_key = ?2",
                rusqlite::params![tap_id, cache_key],
            )
            .map_err(io::Error::other)?;
            Ok(())
        })
        .await
        .map_err(io::Error::other)?
    }

    /// Increment `use_count` and update `last_used_at` for an entry.
    pub async fn touch(&self, tap_id: String, cache_key: String) -> io::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<()> {
            let now = chrono::Utc::now().timestamp();
            let conn = conn.lock().map_err(|_| io::Error::other("lock poisoned"))?;
            conn.execute(
                "UPDATE cache_entries
                 SET use_count = use_count + 1, last_used_at = ?3
                 WHERE tap_id = ?1 AND cache_key = ?2",
                rusqlite::params![tap_id, cache_key, now],
            )
            .map_err(io::Error::other)?;
            Ok(())
        })
        .await
        .map_err(io::Error::other)?
    }

    /// Set the GDSF eviction priority for an entry.
    pub async fn set_gdsf_priority(
        &self,
        tap_id: String,
        cache_key: String,
        priority: f64,
    ) -> io::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<()> {
            let conn = conn.lock().map_err(|_| io::Error::other("lock poisoned"))?;
            conn.execute(
                "UPDATE cache_entries SET gdsf_priority = ?3 WHERE tap_id = ?1 AND cache_key = ?2",
                rusqlite::params![tap_id, cache_key, priority],
            )
            .map_err(io::Error::other)?;
            Ok(())
        })
        .await
        .map_err(io::Error::other)?
    }

    /// Return every row in the table (all states, with or without opus_path).
    pub async fn get_all_entries(&self) -> io::Result<Vec<DbEntry>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<Vec<DbEntry>> {
            let conn = conn.lock().map_err(|_| io::Error::other("lock poisoned"))?;
            let mut stmt = conn
                .prepare(
                    "SELECT tap_id, cache_key, opus_path, json_path, expire_at, use_count, last_used_at,
                            created_at, gdsf_priority, is_downloading
                     FROM cache_entries",
                )
                .map_err(io::Error::other)?;

            let rows = stmt
                .query_map([], |row| {
                    row_to_entry(row).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
                    })
                })
                .map_err(io::Error::other)?;

            let mut entries = Vec::new();
            for row in rows {
                entries.push(row.map_err(io::Error::other)?);
            }
            Ok(entries)
        })
        .await
        .map_err(io::Error::other)?
    }

    /// Return all `opus_path` values for complete (non-downloading) entries.
    pub async fn get_all_opus_paths(&self) -> io::Result<Vec<String>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<Vec<String>> {
            let conn = conn.lock().map_err(|_| io::Error::other("lock poisoned"))?;
            let mut stmt = conn
                .prepare(
                    "SELECT opus_path FROM cache_entries
                     WHERE opus_path IS NOT NULL AND is_downloading = 0",
                )
                .map_err(io::Error::other)?;

            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(io::Error::other)?;

            let mut paths = Vec::new();
            for row in rows {
                paths.push(row.map_err(io::Error::other)?);
            }
            Ok(paths)
        })
        .await
        .map_err(io::Error::other)?
    }

    /// Return all `json_path` values (for all entries).
    pub async fn get_all_json_paths(&self) -> io::Result<Vec<String>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<Vec<String>> {
            let conn = conn.lock().map_err(|_| io::Error::other("lock poisoned"))?;
            let mut stmt = conn
                .prepare("SELECT json_path FROM cache_entries WHERE json_path != ''")
                .map_err(io::Error::other)?;

            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(io::Error::other)?;

            let mut paths = Vec::new();
            for row in rows {
                paths.push(row.map_err(io::Error::other)?);
            }
            Ok(paths)
        })
        .await
        .map_err(io::Error::other)?
    }

    /// Return up to `limit` entries with the lowest GDSF priority (eviction candidates).
    /// Only returns complete (non-downloading), non-metadata entries that have an opus file.
    pub async fn get_lowest_priority_entries(&self, limit: usize) -> io::Result<Vec<DbEntry>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<Vec<DbEntry>> {
            let conn = conn.lock().map_err(|_| io::Error::other("lock poisoned"))?;
            let mut stmt = conn
                .prepare(
                    "SELECT tap_id, cache_key, opus_path, json_path, expire_at, use_count, last_used_at,
                            created_at, gdsf_priority, is_downloading
                     FROM cache_entries
                     WHERE is_downloading = 0 AND opus_path IS NOT NULL
                     ORDER BY gdsf_priority ASC
                     LIMIT ?1",
                )
                .map_err(io::Error::other)?;

            let rows = stmt
                .query_map(rusqlite::params![limit as i64], |row| {
                    row_to_entry(row).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
                    })
                })
                .map_err(io::Error::other)?;

            let mut entries = Vec::new();
            for row in rows {
                entries.push(row.map_err(io::Error::other)?);
            }
            Ok(entries)
        })
        .await
        .map_err(io::Error::other)?
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a rusqlite error to an io::Error, preserving the message.
fn rusqlite_to_io(e: rusqlite::Error, _ctx: &str) -> io::Error {
    io::Error::other(e)
}

/// Returns true if the error indicates a corrupt or unreadable SQLite database
/// that should be deleted and recreated.
fn is_corrupt_error(e: &io::Error) -> bool {
    let msg = e.to_string();
    // SQLITE_CORRUPT (11): "database disk image is malformed"
    // SQLITE_NOTADB  (26): "file is not a database"
    msg.contains("malformed")
        || msg.contains("disk image")
        || msg.contains("not a database")
        || msg.contains("file is not a database")
}

fn row_to_entry(row: &rusqlite::Row<'_>) -> io::Result<DbEntry> {
    Ok(DbEntry {
        tap_id: row.get(0).map_err(io::Error::other)?,
        cache_key: row.get(1).map_err(io::Error::other)?,
        opus_path: row.get(2).map_err(io::Error::other)?,
        json_path: row.get(3).map_err(io::Error::other)?,
        expire_at: row.get(4).map_err(io::Error::other)?,
        use_count: row.get(5).map_err(io::Error::other)?,
        last_used_at: row.get(6).map_err(io::Error::other)?,
        created_at: row.get(7).map_err(io::Error::other)?,
        gdsf_priority: row.get(8).map_err(io::Error::other)?,
        is_downloading: {
            let v: i64 = row.get(9).map_err(io::Error::other)?;
            v != 0
        },
    })
}
