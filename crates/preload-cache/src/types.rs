use bytes::Bytes;
use zako3_types::{AudioCachePolicy, AudioMetadata, cache::AudioCacheItem};

// ---------------------------------------------------------------------------
// PreloadId
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreloadId(pub u64);

// ---------------------------------------------------------------------------
// CacheEntryKind / CacheEntry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum CacheEntryKind {
    /// Entry has an associated `.opus` audio file.
    Audio { is_downloading: bool },
    /// Entry stores only metadata; no `.opus` file.
    Metadata,
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub item: AudioCacheItem,
    pub metadatas: Vec<AudioMetadata>,
    pub cache_key: AudioCachePolicy,
    pub kind: CacheEntryKind,
}

impl CacheEntry {
    pub fn has_audio(&self) -> bool {
        matches!(self.kind, CacheEntryKind::Audio { .. })
    }

    pub fn is_downloading(&self) -> bool {
        matches!(self.kind, CacheEntryKind::Audio { is_downloading: true })
    }
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
