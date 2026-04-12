use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use zako3_types::{hq::*, *};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioEngineError {
    PermissionDenied,
    AlreadyJoined,
    NotJoined,
    InternalError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioEngineCommandResponse {
    Ok,
    SessionState(SessionState),
    DiscordVoiceState(Vec<SessionInfo>),
    Error(AudioEngineError),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub guild_id: GuildId,
    pub channel_id: ChannelId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioEngineCommandRequest {
    pub session: Option<SessionInfo>,
    pub command: AudioEngineCommand,
    pub headers: HashMap<String, String>,
    pub idempotency_key: Option<String>,
}

#[jsonrpsee::proc_macros::rpc(server, client)]
pub trait TrafficLightRpc {
    #[method(name = "execute")]
    async fn execute(&self, request: AudioEngineCommandRequest) -> jsonrpsee::core::RpcResult<AudioEngineCommandResponse>;

    #[method(name = "get_sessions_in_guild")]
    async fn get_sessions_in_guild(&self, guild_id: GuildId) -> jsonrpsee::core::RpcResult<Vec<SessionState>>;

    #[method(name = "report_guilds")]
    async fn report_guilds(&self, token: String, guilds: Vec<GuildId>) -> jsonrpsee::core::RpcResult<()>;
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
    FetchDiscordVoiceState,
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
