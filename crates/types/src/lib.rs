use derive_more::{Display, From, FromStr, Into};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use zakofish::types::{
    AttachedMetadata, AudioCachePolicy, AudioCacheType, AudioMetadata, AudioRequestString,
};

pub mod taphub;
pub use taphub::*;

pub mod error;
pub use error::*;

pub mod session_state;
pub use session_state::*;

pub mod cache;
pub use cache::*;
use utoipa::ToSchema;

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

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Into, From, Display, FromStr, Serialize, Deserialize,
)]
#[display("{_0}")]
pub struct UserId(pub String);

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Into,
    From,
    FromStr,
    Display,
    Serialize,
    Deserialize,
    ToSchema,
)]
#[display("{_0}")]
pub struct TapName(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Into, From, Display, Serialize, Deserialize)]
#[display("{_0}")]
pub struct Volume(f32);

pub struct AudioResponse {
    pub cache_key: Option<AudioCachePolicy>,
    pub metadatas: Vec<AudioMetadata>,
    pub stream: tokio::sync::mpsc::Receiver<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioStopFilter {
    All,
    Music,
    TTS(UserId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRequest {
    pub tap_id: hq::TapId,
    pub request: AudioRequestString,
    pub discord_user_id: hq::DiscordUserId,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedAudioRequest {
    pub tap_id: hq::TapId,
    pub audio_request: AudioRequestString,
    pub cache_key: AudioCachePolicy,
    pub discord_user_id: hq::DiscordUserId,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub track_id: TrackId,
    pub metadatas: Vec<AudioMetadata>,
    pub request: CachedAudioRequest,
    pub volume: Volume,
    pub queue_name: QueueName,
    pub paused: bool,
    /// When this track entered the queue, as unix milliseconds.
    ///
    /// The engine uses it to drop stale TTS before it is played: a busy user's
    /// queue can otherwise hold a message for a minute or two and then play it
    /// long after it was said, which is worse than not playing it at all.
    ///
    /// `None` means "unknown": state written by an engine that predates this
    /// field, or a track restored from such state. An unknown age is never
    /// treated as stale — silently dropping audio we cannot date is the worse
    /// failure, so an undated track plays.
    #[serde(default)]
    pub queued_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMetaResponse {
    pub metadatas: Vec<AudioMetadata>,
    pub cache_key: AudioCachePolicy,
    pub base_volume: f32,
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
