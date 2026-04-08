use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AudioMetadataDto {
    pub r#type: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrackDto {
    pub track_id: String,
    pub queue_name: String,
    pub metadata: Vec<AudioMetadataDto>,
    pub tap_name: String,
    pub audio_request_string: String,
    pub requested_by: String,
    pub volume: f32,
    pub paused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GuildPlaybackStateDto {
    pub guild_id: String,
    pub guild_name: String,
    pub guild_icon_url: Option<String>,
    pub channel_id: String,
    pub channel_name: String,
    pub queues: HashMap<String, Vec<TrackDto>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackActionDto {
    pub id: String,
    pub action_type: String,
    pub guild_id: String,
    pub channel_id: String,
    pub actor_discord_user_id: String,
    pub created_at: DateTime<Utc>,
    pub undone_at: Option<DateTime<Utc>>,
    pub undone_by_discord_user_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StopTrackDto {
    pub guild_id: String,
    pub channel_id: String,
    pub track_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SkipDto {
    pub guild_id: String,
    pub channel_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct QueueOperation {
    /// "remove" or "set_volume"
    pub op: String,
    pub track_id: String,
    pub target_index: Option<usize>,
    pub volume: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EditQueueDto {
    pub guild_id: String,
    pub channel_id: String,
    pub operations: Vec<QueueOperation>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PauseTrackDto {
    pub guild_id: String,
    pub channel_id: String,
    pub track_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResumeTrackDto {
    pub guild_id: String,
    pub channel_id: String,
    pub track_id: String,
}
