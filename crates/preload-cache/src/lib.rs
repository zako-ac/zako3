use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use sha2::{Digest, Sha256};
use tokio::{
    fs,
    io::{self, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::{mpsc, oneshot},
};
use tracing::warn;
use zako3_types::{
    AudioCachePolicy, AudioMetadata,
    cache::{AudioCacheItem, AudioCacheItemKey},
    hq::TapId,
};

// ---------------------------------------------------------------------------
// PreloadId
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreloadId(pub u64);

// ---------------------------------------------------------------------------
// CacheEntry — stored as JSON alongside the .opus file
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheEntry {
    pub item: AudioCacheItem,
    pub metadatas: Vec<AudioMetadata>,
    pub cache_key: AudioCachePolicy,
}

// ---------------------------------------------------------------------------
// AudioPreload — file-backed Opus frame writer + reader
// ---------------------------------------------------------------------------

pub struct AudioPreload {
    dir: PathBuf,
    max_file_bytes: Option<u64>,
}

impl AudioPreload {
    pub fn new(dir: PathBuf, max_file_bytes: Option<u64>) -> Self {
        Self { dir, max_file_bytes }
    }

    pub(crate) fn frame_path(&self, id: PreloadId) -> PathBuf {
        self.dir.join(format!("{}.opus", id.0))
    }

    fn lock_path(&self, id: PreloadId) -> PathBuf {
        self.dir.join(format!("{}.lock", id.0))
    }

    /// Spawns a task that drains `stream` and writes frames to disk.
    /// Returns immediately. Any I/O error during writing deletes both files.
    pub fn preload(&self, id: PreloadId, stream: mpsc::Receiver<Bytes>) {
        let frame_path = self.frame_path(id);
        let lock_path = self.lock_path(id);
        tokio::spawn(write_task(frame_path, lock_path, stream, self.max_file_bytes));
    }

    /// Deletes the frame file and lock file for `id` if they exist.
    pub async fn delete_preload(&self, id: PreloadId) -> io::Result<()> {
        remove_if_exists(&self.frame_path(id)).await?;
        remove_if_exists(&self.lock_path(id)).await?;
        Ok(())
    }

    /// Opens a reader for `id`. Returns `None` if the frame file doesn't exist yet.
    pub async fn open_reader(&self, id: PreloadId) -> Option<PreloadReader> {
        let frame_path = self.frame_path(id);
        let lock_path = self.lock_path(id);
        let file = fs::File::open(&frame_path).await.ok()?;
        Some(PreloadReader {
            file: BufReader::new(file),
            lock_path,
        })
    }
}

// ---------------------------------------------------------------------------
// Write task
// ---------------------------------------------------------------------------

async fn write_task(
    frame_path: PathBuf,
    lock_path: PathBuf,
    mut stream: mpsc::Receiver<Bytes>,
    max_file_bytes: Option<u64>,
) {
    let result = async {
        let mut file = fs::File::create(&frame_path).await?;
        fs::File::create(&lock_path).await?;

        let mut total_bytes: u64 = 0;
        while let Some(frame) = stream.recv().await {
            total_bytes += 4 + frame.len() as u64;
            if let Some(max) = max_file_bytes {
                if total_bytes > max {
                    warn!("preload exceeded max_file_bytes ({max}), dropping");
                    drop(stream);
                    return Err(io::Error::new(
                        io::ErrorKind::FileTooLarge,
                        "preload exceeded max_file_bytes",
                    ));
                }
            }
            let len = frame.len() as u32;
            file.write_all(&len.to_le_bytes()).await?;
            file.write_all(&frame).await?;
        }

        file.flush().await?;
        file.sync_data().await?;
        fs::remove_file(&lock_path).await?;

        io::Result::Ok(())
    }
    .await;

    if let Err(e) = result {
        warn!(?e, "preload write failed, cleaning up");
        let _ = fs::remove_file(&frame_path).await;
        let _ = fs::remove_file(&lock_path).await;
    }
}

// ---------------------------------------------------------------------------
// PreloadReader
// ---------------------------------------------------------------------------

pub struct PreloadReader {
    file: BufReader<fs::File>,
    lock_path: PathBuf,
}

pub enum NextFrame {
    /// A complete Opus frame.
    Frame(Bytes),
    /// Writer is still in progress; caller should sleep and retry.
    Pending,
    /// Writing is complete and all frames have been consumed.
    Done,
}

impl PreloadReader {
    pub async fn next_frame(&mut self) -> io::Result<NextFrame> {
        let mut len_buf = [0u8; 4];
        match self.file.read_exact(&mut len_buf).await {
            Ok(_) => {
                let frame_len = u32::from_le_bytes(len_buf) as usize;
                let mut buf = vec![0u8; frame_len];
                self.file.read_exact(&mut buf).await?;
                Ok(NextFrame::Frame(Bytes::from(buf)))
            }
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                if self.lock_path.exists() {
                    Ok(NextFrame::Pending)
                } else {
                    Ok(NextFrame::Done)
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Call after receiving `NextFrame::Done` to execute the end action.
    /// Consumes the reader to prevent double-finalization.
    pub async fn finalize(
        self,
        preload_id: PreloadId,
        preload: &AudioPreload,
        action: PreloadReadEndAction,
    ) -> io::Result<()> {
        match action {
            PreloadReadEndAction::Delete => {
                preload.delete_preload(preload_id).await?;
            }
            PreloadReadEndAction::MoveToCache {
                item,
                metadatas,
                cache_key,
                cache,
            } => {
                let opus_path = preload.frame_path(preload_id);
                cache
                    .store_from_path(item, metadatas, cache_key, &opus_path)
                    .await?;
                preload.delete_preload(preload_id).await?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PreloadReadEndAction
// ---------------------------------------------------------------------------

pub enum PreloadReadEndAction {
    /// Delete the preload files when reading completes successfully.
    Delete,
    /// Move the preload .opus file into the cache, then remove preload files.
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

    /// Move/rename a preload .opus file into the cache.
    /// Falls back to copy+delete if rename fails (cross-device).
    async fn store_from_path(
        &self,
        item: AudioCacheItem,
        metadatas: Vec<AudioMetadata>,
        cache_key: AudioCachePolicy,
        opus_path: &PathBuf,
    ) -> io::Result<()>;

    /// Open a reader for cached audio. Returns `None` if not cached or expired.
    async fn open_reader(&self, tap_id: &TapId, key: &AudioCacheItemKey) -> Option<PreloadReader>;

    /// Read the full cache entry (item + metadatas + cache_key). Returns `None` if not found or expired.
    async fn get_entry(&self, tap_id: &TapId, key: &AudioCacheItemKey) -> Option<CacheEntry>;

    /// Write only the metadata JSON for an item (no audio frames / no .opus file).
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
}

impl FileAudioCache {
    pub fn new(dir: PathBuf, max_file_bytes: Option<u64>) -> Self {
        Self { dir, max_file_bytes }
    }

    /// Filename stem: `{tap_id}-{SHA-256(serde_json(key)) as hex}`
    fn cache_stem(tap_id: &TapId, key: &AudioCacheItemKey) -> String {
        let key_json =
            serde_json::to_string(key).expect("AudioCacheItemKey is always serializable");
        let hash = hex::encode(Sha256::digest(key_json.as_bytes()));
        format!("{}-{}", tap_id, hash)
    }

    fn opus_path(&self, stem: &str) -> PathBuf {
        self.dir.join(format!("{stem}.opus"))
    }

    fn json_path(&self, stem: &str) -> PathBuf {
        self.dir.join(format!("{stem}.json"))
    }

    fn is_expired(entry: &CacheEntry) -> bool {
        entry
            .item
            .expire_at
            .map(|t| Utc::now() >= t)
            .unwrap_or(false)
    }
}

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
        let stem = Self::cache_stem(&item.tap_id, &item.key);
        let opus_path = self.opus_path(&stem);
        let json_path = self.json_path(&stem);

        let result = async {
            let mut file = fs::File::create(&opus_path).await?;
            let mut total_bytes: u64 = 0;
            while let Some(frame) = stream.recv().await {
                total_bytes += 4 + frame.len() as u64;
                if let Some(max) = self.max_file_bytes {
                    if total_bytes > max {
                        warn!(tap_id = %item.tap_id, "cache store exceeded max_file_bytes ({max}), dropping");
                        return Err(io::Error::new(
                            io::ErrorKind::FileTooLarge,
                            "cache store exceeded max_file_bytes",
                        ));
                    }
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
                    "reliable stream ended early; discarding partial audio cache",
                ));
            }

            let entry = CacheEntry { item, metadatas, cache_key };
            let json = serde_json::to_vec(&entry)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            fs::write(&json_path, json).await?;
            tracing::info!(tap_id = %entry.item.tap_id, key = %entry.item.key, "audio cached successfully");

            io::Result::Ok(())
        }
        .await;

        if let Err(e) = result {
            let _ = fs::remove_file(&opus_path).await;
            let _ = fs::remove_file(&json_path).await;
            return Err(e);
        }
        Ok(())
    }

    async fn store_from_path(
        &self,
        item: AudioCacheItem,
        metadatas: Vec<AudioMetadata>,
        cache_key: AudioCachePolicy,
        opus_path: &PathBuf,
    ) -> io::Result<()> {
        let stem = Self::cache_stem(&item.tap_id, &item.key);
        let dest_opus = self.opus_path(&stem);
        let dest_json = self.json_path(&stem);

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

        // Try atomic rename first; fall back to copy+delete on cross-device error.
        if let Err(e) = fs::rename(opus_path, &dest_opus).await {
            // 18 = EXDEV (invalid cross-device link)
            if e.raw_os_error() == Some(18) {
                fs::copy(opus_path, &dest_opus).await?;
                let _ = fs::remove_file(opus_path).await;
            } else {
                return Err(e);
            }
        }

        let entry = CacheEntry { item, metadatas, cache_key };
        let json = serde_json::to_vec(&entry)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if let Err(e) = fs::write(&dest_json, json).await {
            let _ = fs::remove_file(&dest_opus).await;
            return Err(e);
        }

        Ok(())
    }

    async fn open_reader(&self, tap_id: &TapId, key: &AudioCacheItemKey) -> Option<PreloadReader> {
        let entry = self.get_entry(tap_id, key).await?;
        if Self::is_expired(&entry) {
            return None;
        }
        let stem = Self::cache_stem(tap_id, key);
        let opus_path = self.opus_path(&stem);
        let file = fs::File::open(&opus_path).await.ok()?;
        // Cache files are fully written — no lock file. Empty sentinel path ensures
        // next_frame always returns Done at EOF.
        Some(PreloadReader {
            file: BufReader::new(file),
            lock_path: PathBuf::from(""),
        })
    }

    async fn get_entry(&self, tap_id: &TapId, key: &AudioCacheItemKey) -> Option<CacheEntry> {
        let stem = Self::cache_stem(tap_id, key);
        let json_path = self.json_path(&stem);
        let bytes = fs::read(&json_path).await.ok()?;
        let entry: CacheEntry = serde_json::from_slice(&bytes).ok()?;
        if Self::is_expired(&entry) {
            return None;
        }
        Some(entry)
    }

    async fn store_metadata(
        &self,
        item: AudioCacheItem,
        metadatas: Vec<AudioMetadata>,
        cache_key: AudioCachePolicy,
    ) -> io::Result<()> {
        let stem = Self::cache_stem(&item.tap_id, &item.key);
        let json_path = self.json_path(&stem);
        let entry = CacheEntry { item, metadatas, cache_key };
        let json = serde_json::to_vec(&entry)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(&json_path, json).await
    }

    async fn delete(&self, tap_id: &TapId, key: &AudioCacheItemKey) -> io::Result<()> {
        let stem = Self::cache_stem(tap_id, key);
        remove_if_exists(&self.opus_path(&stem)).await?;
        remove_if_exists(&self.json_path(&stem)).await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn remove_if_exists(path: &PathBuf) -> io::Result<()> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
