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
