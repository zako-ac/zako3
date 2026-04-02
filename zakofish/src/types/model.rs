use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioCacheType {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "ar_hash")]
    ARHash,
    #[serde(rename = "key")]
    CacheKey(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioCachePolicy {
    pub cache_type: AudioCacheType,
    pub ttl_seconds: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum AudioMetadata {
    Title(String),
    Description(String),
    Artist(String),
    Album(String),
    ImageUrl(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HubRejectReasonType {
    #[serde(rename = "unauthorized")]
    Unauthorized,

    #[serde(rename = "internal_error")]
    InternalError,
}
