use std::collections::HashMap;

use anyhow::{Context as _, Result};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use opentelemetry::global;
use thiserror::Error;
use tl_protocol::{
    AudioEngineCommand, AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineError,
    AudioEngineSessionCommand, AudioPlayRequest, SessionInfo, TrafficLightRpcClient,
};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use zako3_types::{
    AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName, SessionState, TapName,
    TrackId, Volume, hq::DiscordUserId,
};

#[derive(Debug, Error)]
pub enum TlClientError {
    #[error("Already in VC")]
    AlreadyJoined,
    #[error("Not in VC")]
    NotJoined,
    #[error("Permission denied")]
    PermissionDenied,
    #[error("{0}")]
    Transport(anyhow::Error),
}

pub struct TlClient {
    client: HttpClient,
}

impl TlClient {
    /// Connect to a TL instance at `url` (e.g. `"http://127.0.0.1:7070"`).
    pub async fn connect(url: &str) -> Result<Self> {
        let client = HttpClientBuilder::default()
            .build(url)
            .with_context(|| format!("failed to build TL client for {url}"))?;
        Ok(Self { client })
    }

    fn map_err(e: jsonrpsee::core::client::Error) -> TlClientError {
        TlClientError::Transport(anyhow::anyhow!("{e}"))
    }

    fn ok_or_err(resp: AudioEngineCommandResponse) -> Result<(), TlClientError> {
        match resp {
            AudioEngineCommandResponse::Ok => Ok(()),
            AudioEngineCommandResponse::Error(AudioEngineError::AlreadyJoined) => {
                Err(TlClientError::AlreadyJoined)
            }
            AudioEngineCommandResponse::Error(AudioEngineError::NotJoined) => {
                Err(TlClientError::NotJoined)
            }
            AudioEngineCommandResponse::Error(AudioEngineError::PermissionDenied) => {
                Err(TlClientError::PermissionDenied)
            }
            AudioEngineCommandResponse::Error(AudioEngineError::InternalError(msg)) => {
                Err(TlClientError::Transport(anyhow::anyhow!("{msg}")))
            }
            other => Err(TlClientError::Transport(anyhow::anyhow!(
                "unexpected response: {other:?}"
            ))),
        }
    }

    fn build_req(
        guild_id: GuildId,
        channel_id: ChannelId,
        command: AudioEngineCommand,
    ) -> AudioEngineCommandRequest {
        let mut headers = HashMap::new();
        let cx = tracing::Span::current().context();
        global::get_text_map_propagator(|p| p.inject_context(&cx, &mut headers));
        AudioEngineCommandRequest {
            session: Some(SessionInfo {
                guild_id,
                channel_id,
            }),
            command,
            headers,
            idempotency_key: None,
        }
    }

    pub async fn join(&self, guild_id: GuildId, channel_id: ChannelId) -> Result<(), TlClientError> {
        let req = Self::build_req(guild_id, channel_id, AudioEngineCommand::Join);
        let resp = self.client.execute(req).await.map_err(Self::map_err)?;
        Self::ok_or_err(resp)
    }

    pub async fn leave(&self, guild_id: GuildId, channel_id: ChannelId) -> Result<(), TlClientError> {
        let req = Self::build_req(
            guild_id,
            channel_id,
            AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Leave),
        );
        let resp = self.client.execute(req).await.map_err(Self::map_err)?;
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
    ) -> Result<(), TlClientError> {
        let req = Self::build_req(
            guild_id,
            channel_id,
            AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Play(
                AudioPlayRequest {
                    queue_name,
                    tap_name,
                    ars,
                    volume,
                    initiator,
                    headers: {
                        let mut h = HashMap::new();
                        let cx = tracing::Span::current().context();
                        global::get_text_map_propagator(|p| p.inject_context(&cx, &mut h));
                        h
                    },
                },
            )),
        );
        let resp = self.client.execute(req).await.map_err(Self::map_err)?;
        Self::ok_or_err(resp)
    }

    pub async fn stop(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        track_id: TrackId,
    ) -> Result<(), TlClientError> {
        let req = Self::build_req(
            guild_id,
            channel_id,
            AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Stop(track_id)),
        );
        let resp = self.client.execute(req).await.map_err(Self::map_err)?;
        Self::ok_or_err(resp)
    }

    pub async fn stop_many(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        filter: AudioStopFilter,
    ) -> Result<(), TlClientError> {
        let req = Self::build_req(
            guild_id,
            channel_id,
            AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::StopMany(filter)),
        );
        let resp = self.client.execute(req).await.map_err(Self::map_err)?;
        Self::ok_or_err(resp)
    }

    pub async fn next_music(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> Result<(), TlClientError> {
        let req = Self::build_req(
            guild_id,
            channel_id,
            AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::NextMusic),
        );
        let resp = self.client.execute(req).await.map_err(Self::map_err)?;
        Self::ok_or_err(resp)
    }

    pub async fn pause(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        queue_name: QueueName,
    ) -> Result<(), TlClientError> {
        let req = Self::build_req(
            guild_id,
            channel_id,
            AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Pause(queue_name)),
        );
        let resp = self.client.execute(req).await.map_err(Self::map_err)?;
        Self::ok_or_err(resp)
    }

    pub async fn resume(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        queue_name: QueueName,
    ) -> Result<(), TlClientError> {
        let req = Self::build_req(
            guild_id,
            channel_id,
            AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Resume(queue_name)),
        );
        let resp = self.client.execute(req).await.map_err(Self::map_err)?;
        Self::ok_or_err(resp)
    }

    pub async fn set_volume(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        track_id: TrackId,
        volume: Volume,
    ) -> Result<(), TlClientError> {
        let req = Self::build_req(
            guild_id,
            channel_id,
            AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::SetVolume {
                track_id,
                volume,
            }),
        );
        let resp = self.client.execute(req).await.map_err(Self::map_err)?;
        Self::ok_or_err(resp)
    }

    pub async fn get_sessions_in_guild(
        &self,
        guild_id: GuildId,
    ) -> Result<Vec<SessionState>, TlClientError> {
        self.client
            .get_sessions_in_guild(guild_id)
            .await
            .map_err(Self::map_err)
    }

    pub async fn report_guilds(
        &self,
        token: String,
        guilds: Vec<GuildId>,
    ) -> Result<(), TlClientError> {
        self.client
            .report_guilds(token, guilds)
            .await
            .map_err(Self::map_err)
    }

    pub async fn register_ae(&self, listen_addr: String) -> Result<String, TlClientError> {
        self.client
            .register_ae(listen_addr)
            .await
            .map_err(Self::map_err)
    }

    pub async fn heartbeat_ae(&self, token: String, listen_addr: String) -> Result<(), TlClientError> {
        self.client
            .heartbeat_ae(token, listen_addr)
            .await
            .map_err(Self::map_err)
    }

    pub async fn get_session_state(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> Result<SessionState, TlClientError> {
        let req = Self::build_req(
            guild_id,
            channel_id,
            AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::GetSessionState),
        );
        let resp = self.client.execute(req).await.map_err(Self::map_err)?;
        match resp {
            AudioEngineCommandResponse::SessionState(s) => Ok(s),
            AudioEngineCommandResponse::Error(AudioEngineError::AlreadyJoined) => {
                Err(TlClientError::AlreadyJoined)
            }
            AudioEngineCommandResponse::Error(AudioEngineError::NotJoined) => {
                Err(TlClientError::NotJoined)
            }
            AudioEngineCommandResponse::Error(AudioEngineError::PermissionDenied) => {
                Err(TlClientError::PermissionDenied)
            }
            AudioEngineCommandResponse::Error(AudioEngineError::InternalError(msg)) => {
                Err(TlClientError::Transport(anyhow::anyhow!("{msg}")))
            }
            other => Err(TlClientError::Transport(anyhow::anyhow!(
                "unexpected response: {other:?}"
            ))),
        }
    }
}
