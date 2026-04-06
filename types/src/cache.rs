use chrono::{DateTime, Utc};
use derive_more::Display;
use serde::{Deserialize, Serialize};

use crate::{TrackId, hq::TapId};

#[derive(Debug, Clone, Serialize, Deserialize, Display)]
#[serde(tag = "type", content = "value")]
pub enum AudioCacheItemKey {
    /// Still stores to a file due to preloading.
    NoCache(TrackId),

    ARHash(String),
    CacheKey(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioCacheItem {
    pub key: AudioCacheItemKey,
    pub tap_id: TapId,
    pub expire_at: Option<DateTime<Utc>>,
}
