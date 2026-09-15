//! Control plane between HQ and the audio engines.
//!
//! Two directions live here:
//!
//! * [`AudioEngineRpc`] — HQ → engine. An engine serves `execute`, which is the
//!   whole audio command vocabulary (join/leave/play/stop/...). HQ picks an
//!   engine and calls it; nothing is proxied through a third party.
//! * [`AeRegistryRpc`] — engine → HQ. An engine announces itself (address, the
//!   Discord bot it logs in as) and keeps that alive with a heartbeat, so HQ
//!   always has a live, self-reported set of engines to place sessions on.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use zako3_types::{GuildId, hq::*, *};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioEngineError {
    PermissionDenied,
    AlreadyJoined,
    NotJoined,
    InternalError(String),
    /// Structured TapHub failure surfaced verbatim to the bot so it can map
    /// to a localized user message instead of falling into the generic
    /// "internal error" bucket.
    Tap(zako3_types::TapHubError),
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

/// What an engine tells HQ about itself on register/heartbeat.
///
/// Deliberately self-reported and address-based: an engine is the only process
/// that knows its own reachable address and which Discord bot it logged in as,
/// so HQ stores what it is told rather than deriving it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AeAdvertisement {
    /// Stable engine identity — the StatefulSet pod name in a cluster. HQ keys
    /// its registry on this, so a restart re-registers over the old entry
    /// instead of leaving a stale one behind.
    pub sink_id: String,
    /// Discord user id of the bot this engine is logged in as. This is the id
    /// HQ matches against Discord's voice state to learn which channel the
    /// engine is serving.
    pub client_id: String,
    /// `http://host:port` HQ dispatches commands to.
    pub advertise_addr: String,
}

#[jsonrpsee::proc_macros::rpc(server, client)]
pub trait AudioEngineRpc {
    #[method(name = "execute")]
    async fn execute(
        &self,
        request: AudioEngineCommandRequest,
    ) -> jsonrpsee::core::RpcResult<AudioEngineCommandResponse>;
}

/// The registry surface HQ serves to the audio engines.
#[jsonrpsee::proc_macros::rpc(server, client)]
pub trait AeRegistryRpc {
    /// First contact after an engine has logged into Discord. Idempotent: a
    /// restart re-registers over the previous entry for the same `sink_id`.
    #[method(name = "register_ae")]
    async fn register_ae(&self, advertisement: AeAdvertisement) -> jsonrpsee::core::RpcResult<()>;

    /// Keeps an entry eligible. HQ stops placing sessions on an engine it has
    /// not heard from recently, so a crashed engine drops out on its own.
    #[method(name = "heartbeat_ae")]
    async fn heartbeat_ae(&self, advertisement: AeAdvertisement) -> jsonrpsee::core::RpcResult<()>;

    /// The guilds this engine's bot is currently a member of. Placement refuses
    /// to send a Join to an engine that is not in the target guild.
    #[method(name = "report_guilds")]
    async fn report_guilds(
        &self,
        client_id: String,
        guilds: Vec<GuildId>,
    ) -> jsonrpsee::core::RpcResult<()>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioPlayRequest {
    pub queue_name: QueueName,
    pub tap_id: TapId,
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

impl AudioEngineCommand {
    pub fn operation(&self) -> &'static str {
        match self {
            AudioEngineCommand::Join => "join",
            AudioEngineCommand::FetchDiscordVoiceState => "fetch_discord_voice_state",
            AudioEngineCommand::SessionCommand(sc) => match sc {
                AudioEngineSessionCommand::Leave => "leave",
                AudioEngineSessionCommand::Play(_) => "play",
                AudioEngineSessionCommand::Stop(_) => "stop",
                AudioEngineSessionCommand::StopMany(_) => "stop_many",
                AudioEngineSessionCommand::SetVolume { .. } => "set_volume",
                AudioEngineSessionCommand::NextMusic => "next_music",
                AudioEngineSessionCommand::Pause(_) => "pause",
                AudioEngineSessionCommand::Resume(_) => "resume",
                AudioEngineSessionCommand::GetSessionState => "get_session_state",
            },
        }
    }
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
