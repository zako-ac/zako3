use bytes::Bytes;
use zako3_types::{AudioCachePolicy, AudioMetadata, cache::AudioCacheItem};

// ---------------------------------------------------------------------------
// PreloadId
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreloadId(pub u64);

// ---------------------------------------------------------------------------
// CacheEntry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub item: AudioCacheItem,
    pub metadatas: Vec<AudioMetadata>,
    pub cache_key: AudioCachePolicy,
    /// True while the opus file is still being written to disk.
    pub is_downloading: bool,
}

// ---------------------------------------------------------------------------
// NextFrame
// ---------------------------------------------------------------------------

pub enum NextFrame {
    /// A complete Opus frame.
    Frame(Bytes),
    /// Writer is still in progress; caller should sleep and retry.
    Pending,
    /// Writing is complete and all frames have been consumed.
    Done,
}
