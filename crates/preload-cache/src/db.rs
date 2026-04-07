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
    /// Unix seconds UTC; `None` means no expiry.
    pub expire_at: Option<i64>,
    pub use_count: i64,
    /// Unix seconds UTC.
    pub last_used_at: Option<i64>,
    /// serde_json of `Vec<AudioMetadata>`
    pub metadatas: String,
    /// serde_json of `AudioCachePolicy`
    pub cache_policy: String,
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
        expire_at      INTEGER,
        use_count      INTEGER NOT NULL DEFAULT 0,
        last_used_at   INTEGER,
        metadatas      TEXT    NOT NULL,
        cache_policy   TEXT    NOT NULL,
        created_at     INTEGER NOT NULL,
        gdsf_priority  REAL    NOT NULL DEFAULT 0.0,
        is_downloading INTEGER NOT NULL DEFAULT 0,
        UNIQUE (tap_id, cache_key)
    );
";

impl CacheDb {
    pub async fn open(path: PathBuf) -> io::Result<Self> {
        tokio::task::spawn_blocking(move || -> io::Result<Self> {
            let conn = rusqlite::Connection::open(&path)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            conn.execute_batch(&format!("PRAGMA journal_mode = WAL;\n{}", SCHEMA))
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Ok(Self { conn: Arc::new(Mutex::new(conn)) })
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    }

    /// Insert or replace a cache entry (sets `is_downloading = 1`).
    pub async fn insert(&self, entry: DbEntry) -> io::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<()> {
            let conn = conn.lock().map_err(|_| io::Error::new(io::ErrorKind::Other, "lock poisoned"))?;
            conn.execute(
                "INSERT OR REPLACE INTO cache_entries
                    (tap_id, cache_key, opus_path, expire_at, use_count, last_used_at,
                     metadatas, cache_policy, created_at, gdsf_priority, is_downloading)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    entry.tap_id,
                    entry.cache_key,
                    entry.opus_path,
                    entry.expire_at,
                    entry.use_count,
                    entry.last_used_at,
                    entry.metadatas,
                    entry.cache_policy,
                    entry.created_at,
                    entry.gdsf_priority,
                    entry.is_downloading as i64,
                ],
            )
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Ok(())
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    }

    /// Mark an entry as fully written (`is_downloading = 0`).
    pub async fn mark_complete(&self, tap_id: String, cache_key: String) -> io::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<()> {
            let conn = conn.lock().map_err(|_| io::Error::new(io::ErrorKind::Other, "lock poisoned"))?;
            conn.execute(
                "UPDATE cache_entries SET is_downloading = 0 WHERE tap_id = ?1 AND cache_key = ?2",
                rusqlite::params![tap_id, cache_key],
            )
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Ok(())
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    }

    /// Look up an entry by `(tap_id, cache_key)`.
    pub async fn get(&self, tap_id: String, cache_key: String) -> io::Result<Option<DbEntry>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<Option<DbEntry>> {
            let conn = conn.lock().map_err(|_| io::Error::new(io::ErrorKind::Other, "lock poisoned"))?;
            let mut stmt = conn
                .prepare(
                    "SELECT tap_id, cache_key, opus_path, expire_at, use_count, last_used_at,
                            metadatas, cache_policy, created_at, gdsf_priority, is_downloading
                     FROM cache_entries WHERE tap_id = ?1 AND cache_key = ?2",
                )
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let mut rows = stmt
                .query(rusqlite::params![tap_id, cache_key])
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            if let Some(row) = rows.next().map_err(|e| io::Error::new(io::ErrorKind::Other, e))? {
                Ok(Some(row_to_entry(row)?))
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    }

    /// Delete an entry by `(tap_id, cache_key)`.
    pub async fn delete(&self, tap_id: String, cache_key: String) -> io::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<()> {
            let conn = conn.lock().map_err(|_| io::Error::new(io::ErrorKind::Other, "lock poisoned"))?;
            conn.execute(
                "DELETE FROM cache_entries WHERE tap_id = ?1 AND cache_key = ?2",
                rusqlite::params![tap_id, cache_key],
            )
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Ok(())
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    }

    /// Increment `use_count` and update `last_used_at` for an entry.
    pub async fn touch(&self, tap_id: String, cache_key: String) -> io::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<()> {
            let now = chrono::Utc::now().timestamp();
            let conn = conn.lock().map_err(|_| io::Error::new(io::ErrorKind::Other, "lock poisoned"))?;
            conn.execute(
                "UPDATE cache_entries
                 SET use_count = use_count + 1, last_used_at = ?3
                 WHERE tap_id = ?1 AND cache_key = ?2",
                rusqlite::params![tap_id, cache_key, now],
            )
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Ok(())
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
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
            let conn = conn.lock().map_err(|_| io::Error::new(io::ErrorKind::Other, "lock poisoned"))?;
            conn.execute(
                "UPDATE cache_entries SET gdsf_priority = ?3 WHERE tap_id = ?1 AND cache_key = ?2",
                rusqlite::params![tap_id, cache_key, priority],
            )
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Ok(())
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    }

    /// Return up to `limit` entries with the lowest GDSF priority (eviction candidates).
    /// Only returns complete (non-downloading), non-metadata entries that have an opus file.
    pub async fn get_lowest_priority_entries(&self, limit: usize) -> io::Result<Vec<DbEntry>> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> io::Result<Vec<DbEntry>> {
            let conn = conn.lock().map_err(|_| io::Error::new(io::ErrorKind::Other, "lock poisoned"))?;
            let mut stmt = conn
                .prepare(
                    "SELECT tap_id, cache_key, opus_path, expire_at, use_count, last_used_at,
                            metadatas, cache_policy, created_at, gdsf_priority, is_downloading
                     FROM cache_entries
                     WHERE is_downloading = 0 AND opus_path IS NOT NULL
                     ORDER BY gdsf_priority ASC
                     LIMIT ?1",
                )
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let rows = stmt
                .query_map(rusqlite::params![limit as i64], |row| {
                    Ok(row_to_entry(row).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
                    })?)
                })
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let mut entries = Vec::new();
            for row in rows {
                entries.push(row.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?);
            }
            Ok(entries)
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn row_to_entry(row: &rusqlite::Row<'_>) -> io::Result<DbEntry> {
    Ok(DbEntry {
        tap_id: row.get(0).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
        cache_key: row.get(1).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
        opus_path: row.get(2).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
        expire_at: row.get(3).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
        use_count: row.get(4).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
        last_used_at: row.get(5).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
        metadatas: row.get(6).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
        cache_policy: row.get(7).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
        created_at: row.get(8).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
        gdsf_priority: row.get(9).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
        is_downloading: {
            let v: i64 = row.get(10).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            v != 0
        },
    })
}
