use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::{Mutex, mpsc};
use zako3_preload_cache::{AudioPreload, FileAudioCache, PreloadId, WriteSignal};
use zako3_types::{AudioCachePolicy, AudioMetadata, cache::AudioCacheItem};

/// State carried by every request handler.
///
/// Constructed by the caller rather than inside the router builder, because the
/// UDP ingest task and the session reaper need the same `sessions` and
/// `active_by_key` maps the HTTP handlers use — and so do tests, which cannot
/// otherwise reach them.
#[derive(Clone)]
pub struct AppState {
    pub cache: Arc<FileAudioCache>,
    pub preload: Arc<AudioPreload>,
    pub sessions: Arc<DashMap<u64, Arc<PreloadSession>>>,
    /// Reverse index from `{tap_id}|{key_json}` to the active preload id, so a
    /// concurrent `GET /stream` can find an in-progress preload for the same target.
    ///
    /// An entry left here after its session dies is worse than a leak: every
    /// later `GET /stream` for that key tails a dead partial file instead of
    /// falling through to the committed entry. The reaper exists for this.
    pub active_by_key: Arc<DashMap<String, u64>>,
    pub admin_token: Option<String>,
    /// Progress of the background index warmup, surfaced by `GET /readyz`.
    pub warmup: Arc<super::WarmupState>,
}

impl AppState {
    pub fn new(
        cache: Arc<FileAudioCache>,
        preload: Arc<AudioPreload>,
        admin_token: Option<String>,
        warmup: Arc<super::WarmupState>,
    ) -> Self {
        Self {
            cache,
            preload,
            sessions: Arc::new(DashMap::new()),
            active_by_key: Arc::new(DashMap::new()),
            admin_token,
            warmup,
        }
    }

    /// Staging paths belonging to sessions that are still live.
    ///
    /// The dangling-file sweep deletes any `.opus` with no database row, and a
    /// preload has no row until it commits — so without this it would happily
    /// delete a file that is actively being written.
    pub fn in_flight_paths(&self) -> std::collections::HashSet<PathBuf> {
        let mut out = std::collections::HashSet::new();
        for entry in self.sessions.iter() {
            let opus = self.preload.frame_path(entry.value().preload_id);
            out.insert(opus.with_extension("lock"));
            out.insert(opus);
        }
        out
    }
}

pub fn active_key(tap_id: &str, key_json: &str) -> String {
    format!("{tap_id}|{key_json}")
}

/// One in-flight preload upload, created by `POST /preload` or by the UDP
/// ingest path, and finalized by a commit or an abort.
pub struct PreloadSession {
    pub preload_id: PreloadId,
    pub item: AudioCacheItem,
    pub metadatas: Vec<AudioMetadata>,
    pub cache_key: AudioCachePolicy,
    /// JSON-encoded `AudioCacheItemKey` — matches what `FileAudioCache` uses on disk
    /// and what `EntryQuery::key` carries on the wire.
    pub key_json: String,
    pub signal: Arc<WriteSignal>,
    /// Sender side of the channel feeding `AudioPreload`'s write task.
    ///
    /// Taken by a streaming HTTP upload, or cloned per datagram by the UDP
    /// path. Dropping the last one lets the write task flush and close the file.
    pub sender: Mutex<Option<mpsc::Sender<Bytes>>>,
    /// Milliseconds since the epoch when this session last saw activity.
    ///
    /// An HTTP upload that dies takes its handler future with it, which drops
    /// the sender; a UDP peer that vanishes gives no signal at all. This is how
    /// the reaper tells the difference between slow and gone.
    last_activity_ms: AtomicI64,
}

impl PreloadSession {
    pub fn new(
        preload_id: PreloadId,
        item: AudioCacheItem,
        metadatas: Vec<AudioMetadata>,
        cache_key: AudioCachePolicy,
        key_json: String,
        signal: Arc<WriteSignal>,
        sender: mpsc::Sender<Bytes>,
    ) -> Self {
        Self {
            preload_id,
            item,
            metadatas,
            cache_key,
            key_json,
            signal,
            sender: Mutex::new(Some(sender)),
            last_activity_ms: AtomicI64::new(now_ms()),
        }
    }

    pub fn touch(&self) {
        self.last_activity_ms.store(now_ms(), Ordering::Relaxed);
    }

    pub fn idle_for(&self) -> Duration {
        let last = self.last_activity_ms.load(Ordering::Relaxed);
        Duration::from_millis((now_ms() - last).max(0) as u64)
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
