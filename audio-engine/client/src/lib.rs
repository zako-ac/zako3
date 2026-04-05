use serde::{Deserialize, Serialize};

use zako3_audio_engine_core::types::{
    AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName, SessionState, TapName,
    TrackId, Volume, hq::DiscordUserId,
};

pub mod client;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum AudioEngineRequest {
    Join {
        guild_id: GuildId,
        channel_id: ChannelId,
    },
    Leave {
        guild_id: GuildId,
        channel_id: ChannelId,
    },
    Play {
        guild_id: GuildId,
        queue_name: QueueName,
        tap_name: TapName,
        audio_request_string: AudioRequestString,
        volume: Volume,
        discord_user_id: DiscordUserId,
    },
    SetVolume {
        guild_id: GuildId,
        track_id: TrackId,
        volume: Volume,
    },
    Stop {
        guild_id: GuildId,
        track_id: TrackId,
    },
    StopMany {
        guild_id: GuildId,
        filter: AudioStopFilter,
    },
    NextMusic {
        guild_id: GuildId,
    },
    GetSessionState {
        guild_id: GuildId,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AudioEngineResponse {
    SuccessBool(bool),
    SuccessTrackId(TrackId),
    SuccessSessionState(SessionState),
    Error(String),
}
