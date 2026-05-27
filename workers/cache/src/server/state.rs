use std::sync::Arc;

use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::{Mutex, mpsc};
use zako3_preload_cache::{AudioPreload, FileAudioCache, PreloadId, WriteSignal};
use zako3_types::{AudioCachePolicy, AudioMetadata, cache::AudioCacheItem};

/// State carried by every request handler.
#[derive(Clone)]
pub struct AppState {
    pub cache: Arc<FileAudioCache>,
    pub preload: Arc<AudioPreload>,
    pub sessions: Arc<DashMap<u64, Arc<PreloadSession>>>,
    /// Reverse index from `{tap_id}|{key_json}` to the active preload id, so a
    /// concurrent `GET /stream` can find an in-progress preload for the same target.
    pub active_by_key: Arc<DashMap<String, u64>>,
    pub admin_token: Option<String>,
}

pub fn active_key(tap_id: &str, key_json: &str) -> String {
    format!("{tap_id}|{key_json}")
}

/// One in-flight preload upload, created by `POST /preload` and finalized by
/// `POST /preload/{id}/commit` or `POST /preload/{id}/abort`.
pub struct PreloadSession {
    pub preload_id: PreloadId,
    pub item: AudioCacheItem,
    pub metadatas: Vec<AudioMetadata>,
    pub cache_key: AudioCachePolicy,
    /// JSON-encoded `AudioCacheItemKey` — matches what `FileAudioCache` uses on disk
    /// and what `EntryQuery::key` carries on the wire.
    pub key_json: String,
    pub signal: Arc<WriteSignal>,
    /// Sender side of the channel feeding `AudioPreload`'s write task. Held by
    /// `POST /preload/{id}/frames` while it pumps bytes; dropped when the frames
    /// upload ends to let the write task flush and close the file.
    pub sender: Mutex<Option<mpsc::Sender<Bytes>>>,
}
