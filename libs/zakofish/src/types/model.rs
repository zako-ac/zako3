use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HubRejectReasonType {
    #[serde(rename = "unauthorized")]
    Unauthorized,

    #[serde(rename = "internal_error")]
    InternalError,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TapId(pub String);

impl std::fmt::Display for TapId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for TapId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

impl From<String> for TapId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AudioRequestString(pub String);

impl std::fmt::Display for AudioRequestString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::str::FromStr for AudioRequestString {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

impl From<String> for AudioRequestString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

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
#[serde(rename_all = "snake_case", content = "value")]
pub enum AudioMetadata {
    Title(String),
    Description(String),
    Artist(String),
    Album(String),
    ImageUrl(String),
}
