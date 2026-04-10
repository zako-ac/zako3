use std::collections::HashMap;

use anyhow::{Context as _, Result, bail};
use tarpc::tokio_serde::formats::Json;
use tl_protocol::{
    AudioEngineCommand, AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineError,
    AudioEngineSessionCommand, AudioPlayRequest, SessionInfo, TrafficLightClient,
};
use zako3_types::{
    AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName, SessionState, TapName,
    TrackId, Volume, hq::DiscordUserId,
};

pub struct TlClient {
    inner: TrafficLightClient,
}

impl TlClient {
    /// Connect to a TL instance at `addr` (e.g. `"127.0.0.1:7070"`).
    pub async fn connect(addr: &str) -> Result<Self> {
        let addr: std::net::SocketAddr = addr
            .parse()
            .with_context(|| format!("invalid TL address: {addr}"))?;
        let transport = tarpc::serde_transport::tcp::connect(addr, Json::default)
            .await
            .with_context(|| format!("failed to connect to TL at {addr}"))?;
        let client =
            TrafficLightClient::new(tarpc::client::Config::default(), transport).spawn();
        Ok(Self { inner: client })
    }

    async fn execute(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        command: AudioEngineCommand,
    ) -> Result<AudioEngineCommandResponse> {
        let req = AudioEngineCommandRequest {
            session: SessionInfo {
                guild_id,
                channel_id,
            },
            command,
            headers: HashMap::new(),
            idempotency_key: None,
        };
        self.inner
            .execute(tarpc::context::current(), req)
            .await
            .context("tarpc call failed")
    }

    fn ok_or_err(resp: AudioEngineCommandResponse) -> Result<()> {
        match resp {
            AudioEngineCommandResponse::Ok => Ok(()),
            AudioEngineCommandResponse::Error(AudioEngineError::InternalError(msg)) => {
                bail!("{msg}")
            }
            AudioEngineCommandResponse::Error(AudioEngineError::PermissionDenied) => {
                bail!("permission denied")
            }
            other => bail!("unexpected response: {other:?}"),
        }
    }

    pub async fn join(&self, guild_id: GuildId, channel_id: ChannelId) -> Result<()> {
        let resp = self
            .execute(guild_id, channel_id, AudioEngineCommand::Join)
            .await?;
        Self::ok_or_err(resp)
    }

    pub async fn leave(&self, guild_id: GuildId, channel_id: ChannelId) -> Result<()> {
        let resp = self
            .execute(
                guild_id,
                channel_id,
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Leave),
            )
            .await?;
        Self::ok_or_err(resp)
    }

    pub async fn play(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        queue_name: QueueName,
        tap_name: TapName,
        ars: AudioRequestString,
        volume: Volume,
        initiator: DiscordUserId,
    ) -> Result<()> {
        let resp = self
            .execute(
                guild_id,
                channel_id,
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Play(
                    AudioPlayRequest {
                        queue_name,
                        tap_name,
                        ars,
                        volume,
                        initiator,
                        headers: HashMap::new(),
                    },
                )),
            )
            .await?;
        Self::ok_or_err(resp)
    }

    pub async fn stop(&self, guild_id: GuildId, channel_id: ChannelId, track_id: TrackId) -> Result<()> {
        let resp = self
            .execute(
                guild_id,
                channel_id,
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Stop(track_id)),
            )
            .await?;
        Self::ok_or_err(resp)
    }

    pub async fn stop_many(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        filter: AudioStopFilter,
    ) -> Result<()> {
        let resp = self
            .execute(
                guild_id,
                channel_id,
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::StopMany(filter)),
            )
            .await?;
        Self::ok_or_err(resp)
    }

    pub async fn next_music(&self, guild_id: GuildId, channel_id: ChannelId) -> Result<()> {
        let resp = self
            .execute(
                guild_id,
                channel_id,
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::NextMusic),
            )
            .await?;
        Self::ok_or_err(resp)
    }

    pub async fn pause(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        queue_name: QueueName,
    ) -> Result<()> {
        let resp = self
            .execute(
                guild_id,
                channel_id,
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Pause(queue_name)),
            )
            .await?;
        Self::ok_or_err(resp)
    }

    pub async fn resume(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        queue_name: QueueName,
    ) -> Result<()> {
        let resp = self
            .execute(
                guild_id,
                channel_id,
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Resume(queue_name)),
            )
            .await?;
        Self::ok_or_err(resp)
    }

    pub async fn set_volume(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        track_id: TrackId,
        volume: Volume,
    ) -> Result<()> {
        let resp = self
            .execute(
                guild_id,
                channel_id,
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::SetVolume {
                    track_id,
                    volume,
                }),
            )
            .await?;
        Self::ok_or_err(resp)
    }

    pub async fn get_sessions_in_guild(&self, guild_id: GuildId) -> Result<Vec<SessionState>> {
        self.inner
            .get_sessions_in_guild(tarpc::context::current(), guild_id)
            .await
            .context("tarpc call failed")
    }

    pub async fn report_guilds(&self, token: String, guilds: Vec<GuildId>) -> Result<()> {
        self.inner
            .report_guilds(tarpc::context::current(), token, guilds)
            .await
            .context("tarpc call failed")
    }

    pub async fn get_session_state(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> Result<SessionState> {
        let resp = self
            .execute(
                guild_id,
                channel_id,
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::GetSessionState),
            )
            .await?;
        match resp {
            AudioEngineCommandResponse::SessionState(s) => Ok(s),
            AudioEngineCommandResponse::Error(AudioEngineError::InternalError(msg)) => bail!("{msg}"),
            AudioEngineCommandResponse::Error(AudioEngineError::PermissionDenied) => {
                bail!("permission denied")
            }
            other => bail!("unexpected response: {other:?}"),
        }
    }
}
