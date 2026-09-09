use serde::{Deserialize, Serialize};
use zako3_preload_cache::{CacheEntry, CacheEntryKind};
use zako3_types::{
    AudioCachePolicy, AudioMetadata,
    cache::{AudioCacheItem, AudioCacheItemKey},
    hq::TapId,
};

/// Request body for `POST /preload`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePreloadReq {
    pub item: AudioCacheItem,
    pub metadatas: Vec<AudioMetadata>,
    pub cache_key: AudioCachePolicy,
}

/// Response body for `POST /preload`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreloadCreatedResp {
    pub preload_id: u64,
}

/// Request body for `POST /ingest` — open a UDP ingest slot.
///
/// Same shape as [`CreatePreloadReq`]; the difference is that the cache worker
/// mints a protofish4 ticket and arms its receiver, rather than waiting to be
/// uploaded to over HTTP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIngestReq {
    pub item: AudioCacheItem,
    pub metadatas: Vec<AudioMetadata>,
    pub cache_key: AudioCachePolicy,
}

/// Response body for `POST /ingest`.
///
/// `preload_id` is not decoration: the caller has opened a session that is
/// already shadowing this cache key for `GET /stream`, so if it then fails to
/// find a tap it **must** abort via `POST /preload/{id}/abort` rather than
/// leaving the session for the reaper. Otherwise every read of that key tails a
/// partial file that will never be finished.
#[derive(Clone, Serialize, Deserialize)]
pub struct IngestCreatedResp {
    pub preload_id: u64,
    pub request_id: uuid::Uuid,
    pub encryption_key: [u8; 32],
}

/// Redacted, because the key is the only capability guarding the datagram path
/// and one `?resp` in a handler would put it in the logs forever.
impl std::fmt::Debug for IngestCreatedResp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IngestCreatedResp")
            .field("preload_id", &self.preload_id)
            .field("request_id", &self.request_id)
            .field("encryption_key", &"<redacted>")
            .finish()
    }
}

/// Request body for `POST /ingest/{id}/finalize`.
///
/// A UDP ingest slot is opened before anyone knows what is in the track — the
/// receiver has to be armed before the tap is asked to send. The real metadata
/// arrives with the tap's response, after that, so it is attached here and the
/// commit waits for both this and the last audio frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizeIngestReq {
    pub metadatas: Vec<AudioMetadata>,
    pub cache_key: AudioCachePolicy,
}

/// Request body for `POST /metadata`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreMetadataReq {
    pub item: AudioCacheItem,
    pub metadatas: Vec<AudioMetadata>,
    pub cache_key: AudioCachePolicy,
}

/// Query string for entry/stream/delete endpoints. `key` is the JSON-encoded
/// `AudioCacheItemKey` (same encoding `FileAudioCache` uses on disk).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryQuery {
    pub tap_id: String,
    pub key: String,
}

impl EntryQuery {
    pub fn new(tap_id: &TapId, key: &AudioCacheItemKey) -> Self {
        Self {
            tap_id: tap_id.0.clone(),
            key: serde_json::to_string(key).expect("AudioCacheItemKey is always serializable"),
        }
    }
}

/// Query string for the tap-wide clear endpoint (`DELETE /entries?tap_id`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TapQuery {
    pub tap_id: String,
}

impl TapQuery {
    pub fn new(tap_id: &TapId) -> Self {
        Self {
            tap_id: tap_id.0.clone(),
        }
    }
}

/// Response body for `DELETE /entries` — number of entries removed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearTapResp {
    pub deleted: usize,
}

/// Response body for `DELETE /entry` — whether a matching entry existed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteEntryResp {
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CacheEntryKindDto {
    Audio { is_downloading: bool },
    Metadata,
}

impl From<CacheEntryKind> for CacheEntryKindDto {
    fn from(k: CacheEntryKind) -> Self {
        match k {
            CacheEntryKind::Audio { is_downloading } => Self::Audio { is_downloading },
            CacheEntryKind::Metadata => Self::Metadata,
        }
    }
}

impl From<CacheEntryKindDto> for CacheEntryKind {
    fn from(k: CacheEntryKindDto) -> Self {
        match k {
            CacheEntryKindDto::Audio { is_downloading } => Self::Audio { is_downloading },
            CacheEntryKindDto::Metadata => Self::Metadata,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntryDto {
    pub item: AudioCacheItem,
    pub metadatas: Vec<AudioMetadata>,
    pub cache_key: AudioCachePolicy,
    pub kind: CacheEntryKindDto,
}

impl From<CacheEntry> for CacheEntryDto {
    fn from(e: CacheEntry) -> Self {
        Self {
            item: e.item,
            metadatas: e.metadatas,
            cache_key: e.cache_key,
            kind: e.kind.into(),
        }
    }
}

impl From<CacheEntryDto> for CacheEntry {
    fn from(d: CacheEntryDto) -> Self {
        Self {
            item: d.item,
            metadatas: d.metadatas,
            cache_key: d.cache_key,
            kind: d.kind.into(),
        }
    }
}
