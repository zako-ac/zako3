use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::{fs, io, sync::RwLock};
use zako3_types::{AudioCachePolicy, AudioCacheType, AudioMetadata};

// ---------------------------------------------------------------------------
// MetaSidecar — JSON file written next to each .opus file.
// This is the single source of truth for all cache entry state.
// New fields use #[serde(default)] for backward-compat with old sidecars.
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct MetaSidecar {
    pub tap_id: String,
    /// serde_json of `AudioCacheItemKey`
    pub cache_key: String,
    pub metadatas: Vec<AudioMetadata>,
    pub cache_policy: AudioCachePolicy,
    /// Unix seconds UTC; `None` means no expiry.
    pub expire_at: Option<i64>,
    /// Unix seconds UTC.
    pub created_at: i64,
    #[serde(default)]
    pub use_count: i64,
    #[serde(default)]
    pub last_used_at: Option<i64>,
    #[serde(default)]
    pub gdsf_priority: f64,
    /// True while the .opus file is still being written.
    #[serde(default)]
    pub is_downloading: bool,
    /// True when a companion .opus file exists alongside this .json.
    #[serde(default)]
    pub has_opus: bool,
}

// ---------------------------------------------------------------------------
// DbEntry — public query result type (unchanged API for cache-gc consumers)
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
// CacheDb — in-memory index backed by JSON sidecar files (no SQLite)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct CacheDb {
    // key: (tap_id, cache_key_json)
    entries: Arc<RwLock<HashMap<(String, String), (PathBuf, MetaSidecar)>>>,
}

impl CacheDb {
    /// Build the index by scanning `dir` for `*.json` sidecar files.
    /// Silently removes any legacy `cache.db` SQLite file if found.
    pub async fn open(dir: &Path) -> io::Result<Self> {
        // Migration: remove old SQLite DB if present.
        let db_path = dir.join("cache.db");
        if db_path.exists() {
            if let Err(e) = fs::remove_file(&db_path).await {
                tracing::warn!(path = %db_path.display(), %e, "failed to remove legacy cache.db");
            } else {
                tracing::info!(path = %db_path.display(), "removed legacy SQLite cache.db");
            }
        }

        let mut map = HashMap::new();

        let mut dir_read = match fs::read_dir(dir).await {
            Ok(d) => d,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(Self { entries: Arc::new(RwLock::new(map)) });
            }
            Err(e) => return Err(e),
        };

        while let Some(entry) = dir_read.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match load_sidecar(&path).await {
                Ok(sidecar) => {
                    let key = (sidecar.tap_id.clone(), sidecar.cache_key.clone());
                    map.insert(key, (path, sidecar));
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), %e, "failed to parse sidecar, skipping");
                }
            }
        }

        Ok(Self {
            entries: Arc::new(RwLock::new(map)),
        })
    }

    /// Insert a `DbEntry` into the index, creating a minimal JSON sidecar on disk.
    /// Used by external tools and tests that construct entries without full metadata.
    pub async fn insert(&self, entry: DbEntry) -> io::Result<()> {
        let sidecar = MetaSidecar {
            tap_id: entry.tap_id.clone(),
            cache_key: entry.cache_key.clone(),
            metadatas: vec![],
            cache_policy: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
            expire_at: entry.expire_at,
            created_at: entry.created_at,
            use_count: entry.use_count,
            last_used_at: entry.last_used_at,
            gdsf_priority: entry.gdsf_priority,
            is_downloading: entry.is_downloading,
            has_opus: entry.opus_path.is_some(),
        };
        let json_path = PathBuf::from(&entry.json_path);
        self.insert_sidecar(json_path, sidecar).await
    }

    /// Write `sidecar` to `json_path` and register it in the index.
    /// Used internally by `FileAudioCache` to persist full metadata.
    pub(crate) async fn insert_sidecar(&self, json_path: PathBuf, sidecar: MetaSidecar) -> io::Result<()> {
        write_sidecar(&json_path, &sidecar).await?;
        let key = (sidecar.tap_id.clone(), sidecar.cache_key.clone());
        self.entries.write().await.insert(key, (json_path, sidecar));
        Ok(())
    }

    /// Mark an entry as fully written (`is_downloading = false`, `has_opus = true`).
    pub async fn mark_complete(&self, tap_id: String, cache_key: String) -> io::Result<()> {
        let (path, sidecar) = {
            let mut map = self.entries.write().await;
            let e = map
                .get_mut(&(tap_id, cache_key))
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "entry not found"))?;
            e.1.is_downloading = false;
            e.1.has_opus = true;
            (e.0.clone(), e.1.clone())
        };
        write_sidecar(&path, &sidecar).await
    }

    /// Look up an entry by `(tap_id, cache_key)`.
    pub async fn get(&self, tap_id: String, cache_key: String) -> io::Result<Option<DbEntry>> {
        let map = self.entries.read().await;
        Ok(map.get(&(tap_id, cache_key)).map(|(p, s)| to_db_entry(p, s)))
    }

    /// Look up the full sidecar (includes metadatas and cache_policy).
    pub(crate) async fn get_sidecar(
        &self,
        tap_id: String,
        cache_key: String,
    ) -> Option<MetaSidecar> {
        let map = self.entries.read().await;
        map.get(&(tap_id, cache_key)).map(|(_, s)| s.clone())
    }

    /// Remove an entry from the index. Does **not** delete files (caller's responsibility).
    pub async fn delete(&self, tap_id: String, cache_key: String) -> io::Result<()> {
        self.entries.write().await.remove(&(tap_id, cache_key));
        Ok(())
    }

    /// Increment `use_count` and update `last_used_at`.
    pub async fn touch(&self, tap_id: String, cache_key: String) -> io::Result<()> {
        let now = chrono::Utc::now().timestamp();
        let (path, sidecar) = {
            let mut map = self.entries.write().await;
            let e = map
                .get_mut(&(tap_id, cache_key))
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "entry not found"))?;
            e.1.use_count += 1;
            e.1.last_used_at = Some(now);
            (e.0.clone(), e.1.clone())
        };
        write_sidecar(&path, &sidecar).await
    }

    /// Set the GDSF eviction priority for an entry.
    pub async fn set_gdsf_priority(
        &self,
        tap_id: String,
        cache_key: String,
        priority: f64,
    ) -> io::Result<()> {
        let (path, sidecar) = {
            let mut map = self.entries.write().await;
            let e = map
                .get_mut(&(tap_id, cache_key))
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "entry not found"))?;
            e.1.gdsf_priority = priority;
            (e.0.clone(), e.1.clone())
        };
        write_sidecar(&path, &sidecar).await
    }

    /// Return every entry in the index.
    pub async fn get_all_entries(&self) -> io::Result<Vec<DbEntry>> {
        let map = self.entries.read().await;
        Ok(map.values().map(|(p, s)| to_db_entry(p, s)).collect())
    }

    /// Return all opus paths for complete (non-downloading, has_opus) entries.
    pub async fn get_all_opus_paths(&self) -> io::Result<Vec<String>> {
        let map = self.entries.read().await;
        Ok(map
            .values()
            .filter(|(_, s)| s.has_opus && !s.is_downloading)
            .map(|(p, _)| p.with_extension("opus").to_string_lossy().into_owned())
            .collect())
    }

    /// Return all json_path values.
    pub async fn get_all_json_paths(&self) -> io::Result<Vec<String>> {
        let map = self.entries.read().await;
        Ok(map
            .values()
            .map(|(p, _)| p.to_string_lossy().into_owned())
            .collect())
    }

    /// Return up to `limit` complete entries with the lowest GDSF priority (eviction candidates).
    pub async fn get_lowest_priority_entries(&self, limit: usize) -> io::Result<Vec<DbEntry>> {
        let map = self.entries.read().await;
        let mut candidates: Vec<DbEntry> = map
            .values()
            .filter(|(_, s)| s.has_opus && !s.is_downloading)
            .map(|(p, s)| to_db_entry(p, s))
            .collect();
        candidates.sort_by(|a, b| {
            a.gdsf_priority
                .partial_cmp(&b.gdsf_priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(limit);
        Ok(candidates)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn to_db_entry(json_path: &Path, s: &MetaSidecar) -> DbEntry {
    DbEntry {
        tap_id: s.tap_id.clone(),
        cache_key: s.cache_key.clone(),
        opus_path: if s.has_opus {
            Some(json_path.with_extension("opus").to_string_lossy().into_owned())
        } else {
            None
        },
        json_path: json_path.to_string_lossy().into_owned(),
        expire_at: s.expire_at,
        use_count: s.use_count,
        last_used_at: s.last_used_at,
        created_at: s.created_at,
        gdsf_priority: s.gdsf_priority,
        is_downloading: s.is_downloading,
    }
}

async fn load_sidecar(path: &Path) -> io::Result<MetaSidecar> {
    let bytes = fs::read(path).await?;
    serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub(crate) async fn write_sidecar(path: &Path, sidecar: &MetaSidecar) -> io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let json =
        serde_json::to_vec(sidecar).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut file = fs::File::create(path).await?;
    file.write_all(&json).await?;
    file.flush().await?;
    file.sync_data().await?;
    Ok(())
}
