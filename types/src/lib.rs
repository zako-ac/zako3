use derive_more::{Display, From, FromStr, Into};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncRead;

pub mod taphub;
pub use taphub::*;

pub mod error;
pub use error::*;

pub mod session_state;
pub use session_state::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Into, From, Display, Serialize, Deserialize)]
#[display("{_0}")]
pub struct GuildId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Into, From, Display, Serialize, Deserialize)]
#[display("{_0}")]
pub struct ChannelId(u64);

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Into, From, FromStr, Display, Serialize, Deserialize,
)]
#[display("{_0}")]
pub struct QueueName(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Into, From, Display, Serialize, Deserialize)]
#[display("{_0}")]
pub struct TrackId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Into, From, Display, Serialize, Deserialize)]
#[display("{_0}")]
pub struct UserId(u64);

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Into, From, FromStr, Display, Serialize, Deserialize,
)]
#[display("{_0}")]
pub struct TapName(String);

#[derive(Debug, Clone, Copy, PartialEq, Into, From, Display, Serialize, Deserialize)]
#[display("{_0}")]
pub struct Volume(f32);

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Into, From, FromStr, Display, Serialize, Deserialize,
)]
#[display("{_0}")]
pub struct AudioRequestString(String);

pub struct AudioResponse {
    pub cache_key: Option<AudioCachePolicy>,
    pub metadatas: Vec<AudioMetadata>,
    pub stream: Box<dyn AsyncRead + Send + Unpin + Sync>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioStopFilter {
    All,
    Music,
    TTS(UserId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRequest {
    pub tap_name: TapName,
    pub request: AudioRequestString,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedAudioRequest {
    pub tap_name: TapName,
    pub audio_request: AudioRequestString,
    pub cache_key: AudioCachePolicy,
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
#[serde(rename_all = "snake_case")]
pub enum AudioMetadata {
    Title(String),
    Description(String),
    Artist(String),
    Album(String),
    ImageUrl(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub track_id: TrackId,
    pub metadatas: Vec<AudioMetadata>,
    pub request: CachedAudioRequest,
    pub volume: Volume,
    pub queue_name: QueueName,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMetaResponse {
    pub metadatas: Vec<AudioMetadata>,
    pub cache_key: AudioCachePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionControlCommand {
    Play(AudioRequest),
    SetVolume(TrackId, Volume),
    Stop(TrackId),
    StopMany(AudioStopFilter),
    NextMusic,
    SetPaused(bool),
}

pub mod hq;
