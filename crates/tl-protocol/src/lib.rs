use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use zako3_types::{hq::*, *};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioEngineError {
    PermissionDenied,
    InternalError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioEngineCommandResponse {
    Ok,
    SessionState(SessionState),
    Error(AudioEngineError),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub guild_id: GuildId,
    pub channel_id: ChannelId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioEngineCommandRequest {
    pub session: SessionInfo,
    pub command: AudioEngineCommand,
    pub headers: HashMap<String, String>,
    pub idempotency_key: Option<String>,
}

#[tarpc::service]
pub trait TrafficLight {
    async fn execute(request: AudioEngineCommandRequest) -> AudioEngineCommandResponse;
    async fn get_sessions_in_guild(guild_id: GuildId) -> Vec<SessionState>;
    async fn report_guilds(token: String, guilds: Vec<GuildId>);
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioPlayRequest {
    pub queue_name: QueueName,
    pub tap_name: TapName,
    pub ars: AudioRequestString,
    pub volume: Volume,
    pub initiator: DiscordUserId,
    pub headers: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AudioEngineCommand {
    Join,
    SessionCommand(AudioEngineSessionCommand),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AudioEngineSessionCommand {
    Leave,

    Play(AudioPlayRequest),
    Stop(TrackId),
    StopMany(AudioStopFilter),
    SetVolume { track_id: TrackId, volume: Volume },

    NextMusic,

    Pause(QueueName),
    Resume(QueueName),

    GetSessionState,
}
