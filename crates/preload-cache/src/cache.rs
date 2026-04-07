use std::{path::{Path, PathBuf}, sync::Arc};

use async_trait::async_trait;
use bytes::Bytes;
use tokio::{
    fs,
    io::{self, AsyncWriteExt},
    sync::{mpsc, oneshot},
};
use tracing::warn;
use zako3_types::{
    AudioCachePolicy, AudioMetadata,
    cache::{AudioCacheItem, AudioCacheItemKey},
    hq::TapId,
};

use crate::{
    db::{CacheDb, DbEntry},
    preload::PreloadReader,
    types::{CacheEntry, CacheEntryKind},
};

// ---------------------------------------------------------------------------
// PreloadReadEndAction
// ---------------------------------------------------------------------------

pub enum PreloadReadEndAction {
    /// Delete the preload files when reading completes successfully.
    Delete,
    /// Move the preload `.opus` file into the cache, then remove preload files.
    MoveToCache {
        item: AudioCacheItem,
        metadatas: Vec<AudioMetadata>,
        cache_key: AudioCachePolicy,
        cache: Arc<dyn AudioCache>,
    },
}

// ---------------------------------------------------------------------------
// AudioCache trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait AudioCache: Send + Sync {
    /// Write frames from stream into cache under `item`.
    /// `done` must fire `()` when the stream ends naturally; if it is dropped
    /// without sending, the store is treated as incomplete and cleaned up.
    async fn store(
        &self,
        item: AudioCacheItem,
        metadatas: Vec<AudioMetadata>,
        cache_key: AudioCachePolicy,
        stream: mpsc::Receiver<Bytes>,
        done: oneshot::Receiver<()>,
    ) -> io::Result<()>;

    /// Move/rename a preload `.opus` file into the cache.
    /// Falls back to copy+delete if rename fails (cross-device).
    async fn store_from_path(
        &self,
        item: AudioCacheItem,
        metadatas: Vec<AudioMetadata>,
        cache_key: AudioCachePolicy,
        opus_path: &Path,
    ) -> io::Result<()>;

    /// Open a reader for cached audio. Returns `None` if not cached, expired, or still downloading.
    async fn open_reader(&self, tap_id: &TapId, key: &AudioCacheItemKey) -> Option<PreloadReader>;

    /// Read the full cache entry. Returns `None` if not found or expired.
    async fn get_entry(&self, tap_id: &TapId, key: &AudioCacheItemKey) -> Option<CacheEntry>;

    /// Write only the metadata for an item (no audio frames / no `.opus` file).
    async fn store_metadata(
        &self,
        item: AudioCacheItem,
        metadatas: Vec<AudioMetadata>,
        cache_key: AudioCachePolicy,
    ) -> io::Result<()>;

    /// Delete cached files for the given key.
    async fn delete(&self, tap_id: &TapId, key: &AudioCacheItemKey) -> io::Result<()>;
}

// ---------------------------------------------------------------------------
// FileAudioCache
// ---------------------------------------------------------------------------

pub struct FileAudioCache {
    dir: PathBuf,
    max_file_bytes: Option<u64>,
    db: Arc<CacheDb>,
}

impl FileAudioCache {
    /// Async constructor: opens (or creates) the SQLite DB at `dir/cache.db`.
    pub async fn open(dir: PathBuf, max_file_bytes: Option<u64>) -> io::Result<Self> {
        fs::create_dir_all(&dir).await?;
        let db_path = dir.join("cache.db");
        let db = CacheDb::open(db_path).await?;
        Ok(Self { dir, max_file_bytes, db: Arc::new(db) })
    }

    /// Expose the underlying `CacheDb` for external tools (e.g. cache-gc).
    pub fn db(&self) -> &CacheDb {
        &self.db
    }

    /// Update the GDSF eviction priority for a cached entry.
    /// The caller is responsible for computing the priority value
    /// (e.g., `clock + use_count as f64 / file_size_bytes as f64`).
    pub async fn update_gdsf_priority(
        &self,
        tap_id: &TapId,
        key: &AudioCacheItemKey,
        priority: f64,
    ) -> io::Result<()> {
        let key_json = key_to_json(key);
        self.db.set_gdsf_priority(tap_id.to_string(), key_json, priority).await
    }

    /// Return up to `limit` entries with the lowest GDSF priority, suitable for eviction.
    /// Only returns complete (non-downloading) entries with an opus file.
    pub async fn eviction_candidates(
        &self,
        limit: usize,
    ) -> io::Result<Vec<(AudioCacheItem, f64)>> {
        let entries = self.db.get_lowest_priority_entries(limit).await?;
        let mut result = Vec::with_capacity(entries.len());
        for entry in entries {
            let key: AudioCacheItemKey = serde_json::from_str(&entry.cache_key)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let expire_at = entry.expire_at.map(|ts| {
                chrono::DateTime::from_timestamp(ts, 0)
                    .unwrap_or(chrono::DateTime::UNIX_EPOCH)
                    .with_timezone(&chrono::Utc)
            });
            let item = AudioCacheItem {
                key,
                tap_id: TapId(entry.tap_id),
                expire_at,
            };
            result.push((item, entry.gdsf_priority));
        }
        Ok(result)
    }

    fn new_opus_path(&self) -> PathBuf {
        self.dir.join(format!("{}.opus", uuid::Uuid::new_v4()))
    }
}

// ---------------------------------------------------------------------------
// AudioCache impl
// ---------------------------------------------------------------------------

#[async_trait]
impl AudioCache for FileAudioCache {
    async fn store(
        &self,
        item: AudioCacheItem,
        metadatas: Vec<AudioMetadata>,
        cache_key: AudioCachePolicy,
        mut stream: mpsc::Receiver<Bytes>,
        done: oneshot::Receiver<()>,
    ) -> io::Result<()> {
        let opus_path = self.new_opus_path();
        let key_json = key_to_json(&item.key);

        let db_entry = DbEntry {
            tap_id: item.tap_id.to_string(),
            cache_key: key_json.clone(),
            opus_path: Some(opus_path.to_string_lossy().into_owned()),
            expire_at: item.expire_at.map(|t| t.timestamp()),
            use_count: 0,
            last_used_at: None,
            metadatas: serde_json::to_string(&metadatas)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            cache_policy: serde_json::to_string(&cache_key)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            created_at: chrono::Utc::now().timestamp(),
            gdsf_priority: 0.0,
            is_downloading: true,
        };

        self.db.insert(db_entry).await?;

        let tap_id_str = item.tap_id.to_string();
        let result: io::Result<()> = async {
            let mut file = fs::File::create(&opus_path).await?;
            let mut total_bytes: u64 = 0;
            while let Some(frame) = stream.recv().await {
                total_bytes += 4 + frame.len() as u64;
                if let Some(max) = self.max_file_bytes
                    && total_bytes > max
                {
                    warn!(tap_id = %item.tap_id, "cache store exceeded max_file_bytes ({max}), dropping");
                    return Err(io::Error::new(
                        io::ErrorKind::FileTooLarge,
                        "cache store exceeded max_file_bytes",
                    ));
                }
                let len = frame.len() as u32;
                file.write_all(&len.to_le_bytes()).await?;
                file.write_all(&frame).await?;
            }
            file.flush().await?;
            file.sync_data().await?;

            if done.await.is_err() {
                warn!(tap_id = %item.tap_id, key = %item.key, "stream ended early; discarding partial audio cache");
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "stream ended early; discarding partial audio cache",
                ));
            }

            self.db.mark_complete(tap_id_str.clone(), key_json.clone()).await?;
            tracing::info!(tap_id = %item.tap_id, key = %item.key, "audio cached successfully");
            Ok(())
        }
        .await;

        if let Err(e) = result {
            let _ = fs::remove_file(&opus_path).await;
            let _ = self.db.delete(tap_id_str, key_json).await;
            return Err(e);
        }
        Ok(())
    }

    async fn store_from_path(
        &self,
        item: AudioCacheItem,
        metadatas: Vec<AudioMetadata>,
        cache_key: AudioCachePolicy,
        opus_path: &Path,
    ) -> io::Result<()> {
        if let Some(max) = self.max_file_bytes {
            let size = fs::metadata(opus_path).await?.len();
            if size > max {
                warn!("store_from_path: file size {size} exceeds max_file_bytes {max}, dropping");
                let _ = fs::remove_file(opus_path).await;
                return Err(io::Error::new(
                    io::ErrorKind::FileTooLarge,
                    "audio file exceeds max_file_bytes",
                ));
            }
        }

        let dest_opus = self.new_opus_path();
        let key_json = key_to_json(&item.key);
        let tap_id_str = item.tap_id.to_string();

        let db_entry = DbEntry {
            tap_id: tap_id_str.clone(),
            cache_key: key_json.clone(),
            opus_path: Some(dest_opus.to_string_lossy().into_owned()),
            expire_at: item.expire_at.map(|t| t.timestamp()),
            use_count: 0,
            last_used_at: None,
            metadatas: serde_json::to_string(&metadatas)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            cache_policy: serde_json::to_string(&cache_key)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            created_at: chrono::Utc::now().timestamp(),
            gdsf_priority: 0.0,
            is_downloading: true,
        };

        self.db.insert(db_entry).await?;

        // Try atomic rename first; fall back to copy+delete on cross-device error.
        let rename_result = fs::rename(opus_path, &dest_opus).await;
        if let Err(e) = rename_result {
            if e.raw_os_error() == Some(18) {
                // EXDEV: cross-device link
                if let Err(copy_err) = fs::copy(opus_path, &dest_opus).await {
                    let _ = self.db.delete(tap_id_str, key_json).await;
                    return Err(copy_err);
                }
                let _ = fs::remove_file(opus_path).await;
            } else {
                let _ = self.db.delete(tap_id_str, key_json).await;
                return Err(e);
            }
        }

        if let Err(e) = self.db.mark_complete(tap_id_str.clone(), key_json.clone()).await {
            let _ = fs::remove_file(&dest_opus).await;
            let _ = self.db.delete(tap_id_str, key_json).await;
            return Err(e);
        }

        Ok(())
    }

    async fn open_reader(&self, tap_id: &TapId, key: &AudioCacheItemKey) -> Option<PreloadReader> {
        let entry = self.db.get(tap_id.to_string(), key_to_json(key)).await.ok()??;
        if entry.is_downloading {
            return None;
        }
        if is_expired(entry.expire_at) {
            return None;
        }
        let opus_path = entry.opus_path?;
        let file = fs::File::open(&opus_path).await.ok()?;

        // Update usage stats (best-effort)
        let _ = self.db.touch(tap_id.to_string(), key_to_json(key)).await;

        // Cache files are fully written — no lock file. Empty path ensures
        // next_frame always returns Done at EOF.
        Some(PreloadReader {
            file: tokio::io::BufReader::new(file),
            lock_path: PathBuf::from(""),
        })
    }

    async fn get_entry(&self, tap_id: &TapId, key: &AudioCacheItemKey) -> Option<CacheEntry> {
        let entry = self.db.get(tap_id.to_string(), key_to_json(key)).await.ok()??;
        if is_expired(entry.expire_at) {
            return None;
        }
        let metadatas: Vec<AudioMetadata> = serde_json::from_str(&entry.metadatas).ok()?;
        let cache_key: AudioCachePolicy = serde_json::from_str(&entry.cache_policy).ok()?;
        let item_key: AudioCacheItemKey = serde_json::from_str(&entry.cache_key).ok()?;
        let expire_at = entry.expire_at.and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
            .map(|dt| dt.with_timezone(&chrono::Utc));
        let item = AudioCacheItem {
            key: item_key,
            tap_id: tap_id.clone(),
            expire_at,
        };
        let kind = if entry.opus_path.is_some() {
            CacheEntryKind::Audio { is_downloading: entry.is_downloading }
        } else {
            CacheEntryKind::Metadata
        };
        Some(CacheEntry { item, metadatas, cache_key, kind })
    }

    async fn store_metadata(
        &self,
        item: AudioCacheItem,
        metadatas: Vec<AudioMetadata>,
        cache_key: AudioCachePolicy,
    ) -> io::Result<()> {
        let key_json = key_to_json(&item.key);
        let db_entry = DbEntry {
            tap_id: item.tap_id.to_string(),
            cache_key: key_json,
            opus_path: None,
            expire_at: item.expire_at.map(|t| t.timestamp()),
            use_count: 0,
            last_used_at: None,
            metadatas: serde_json::to_string(&metadatas)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            cache_policy: serde_json::to_string(&cache_key)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            created_at: chrono::Utc::now().timestamp(),
            gdsf_priority: 0.0,
            is_downloading: false,
        };
        self.db.insert(db_entry).await
    }

    async fn delete(&self, tap_id: &TapId, key: &AudioCacheItemKey) -> io::Result<()> {
        let key_json = key_to_json(key);
        if let Ok(Some(entry)) = self.db.get(tap_id.to_string(), key_json.clone()).await
            && let Some(path) = entry.opus_path
        {
            let _ = remove_if_exists(&PathBuf::from(path)).await;
        }
        self.db.delete(tap_id.to_string(), key_json).await
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn key_to_json(key: &AudioCacheItemKey) -> String {
    serde_json::to_string(key).expect("AudioCacheItemKey is always serializable")
}

fn is_expired(expire_at: Option<i64>) -> bool {
    expire_at.map(|ts| chrono::Utc::now().timestamp() >= ts).unwrap_or(false)
}

async fn remove_if_exists(path: &PathBuf) -> io::Result<()> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
