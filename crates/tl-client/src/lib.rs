use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use backon::{ExponentialBuilder, Retryable};
use tarpc::tokio_serde::formats::Json;
use thiserror::Error;
use tl_protocol::{
    AudioEngineCommand, AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineError,
    AudioEngineSessionCommand, AudioPlayRequest, SessionInfo, TrafficLightClient,
};
use tokio::sync::RwLock;
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
    addr: String,
    inner: Arc<RwLock<TrafficLightClient>>,
}

impl TlClient {
    async fn connect_inner(addr: &str) -> Result<TrafficLightClient> {
        let transport = tarpc::serde_transport::tcp::connect(addr, Json::default)
            .await
            .with_context(|| format!("failed to connect to TL at {addr}"))?;
        Ok(TrafficLightClient::new(tarpc::client::Config::default(), transport).spawn())
    }

    /// Connect to a TL instance at `addr` (e.g. `"127.0.0.1:7070"` or `"hostname:7070"`).
    pub async fn connect(addr: &str) -> Result<Self> {
        let client = Self::connect_inner(addr).await?;
        Ok(Self {
            addr: addr.to_string(),
            inner: Arc::new(RwLock::new(client)),
        })
    }

    async fn reconnect(&self) -> Result<(), TlClientError> {
        let addr = self.addr.clone();
        let new_client = (|| {
            let addr = addr.clone();
            async move {
                Self::connect_inner(&addr)
                    .await
                    .map_err(TlClientError::Transport)
            }
        })
        .retry(
            ExponentialBuilder::default()
                .with_min_delay(Duration::from_millis(500))
                .with_max_delay(Duration::from_secs(30))
                .with_max_times(usize::MAX),
        )
        .notify(|err: &TlClientError, dur| {
            tracing::warn!("tl-client: reconnect failed ({err}), retrying in {dur:?}");
        })
        .await?;

        *self.inner.write().await = new_client;
        Ok(())
    }

    /// Call `f` with a cloned client handle. On transport failure, reconnect with
    /// exponential backoff and retry the call once.
    async fn with_reconnect<F, Fut, T>(&self, call: F) -> Result<T, TlClientError>
    where
        F: Fn(TrafficLightClient) -> Fut,
        Fut: std::future::Future<Output = Result<T, tarpc::client::RpcError>>,
    {
        let client = self.inner.read().await.clone();
        match call(client).await {
            Ok(v) => Ok(v),
            Err(e) => {
                tracing::warn!("tl-client: call failed ({e}), reconnecting");
                self.reconnect().await?;
                let client = self.inner.read().await.clone();
                call(client)
                    .await
                    .map_err(|e| TlClientError::Transport(anyhow::anyhow!("{e}")))
            }
        }
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
        AudioEngineCommandRequest {
            session: SessionInfo {
                guild_id,
                channel_id,
            },
            command,
            headers: HashMap::new(),
            idempotency_key: None,
        }
    }

    pub async fn join(&self, guild_id: GuildId, channel_id: ChannelId) -> Result<(), TlClientError> {
        let req = Self::build_req(guild_id, channel_id, AudioEngineCommand::Join);
        let resp = self
            .with_reconnect(|c| {
                let req = req.clone();
                async move { c.execute(tarpc::context::current(), req).await }
            })
            .await?;
        Self::ok_or_err(resp)
    }

    pub async fn leave(&self, guild_id: GuildId, channel_id: ChannelId) -> Result<(), TlClientError> {
        let req = Self::build_req(
            guild_id,
            channel_id,
            AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Leave),
        );
        let resp = self
            .with_reconnect(|c| {
                let req = req.clone();
                async move { c.execute(tarpc::context::current(), req).await }
            })
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
                    headers: HashMap::new(),
                },
            )),
        );
        let resp = self
            .with_reconnect(|c| {
                let req = req.clone();
                async move { c.execute(tarpc::context::current(), req).await }
            })
            .await?;
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
        let resp = self
            .with_reconnect(|c| {
                let req = req.clone();
                async move { c.execute(tarpc::context::current(), req).await }
            })
            .await?;
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
        let resp = self
            .with_reconnect(|c| {
                let req = req.clone();
                async move { c.execute(tarpc::context::current(), req).await }
            })
            .await?;
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
        let resp = self
            .with_reconnect(|c| {
                let req = req.clone();
                async move { c.execute(tarpc::context::current(), req).await }
            })
            .await?;
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
        let resp = self
            .with_reconnect(|c| {
                let req = req.clone();
                async move { c.execute(tarpc::context::current(), req).await }
            })
            .await?;
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
        let resp = self
            .with_reconnect(|c| {
                let req = req.clone();
                async move { c.execute(tarpc::context::current(), req).await }
            })
            .await?;
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
        let resp = self
            .with_reconnect(|c| {
                let req = req.clone();
                async move { c.execute(tarpc::context::current(), req).await }
            })
            .await?;
        Self::ok_or_err(resp)
    }

    pub async fn get_sessions_in_guild(
        &self,
        guild_id: GuildId,
    ) -> Result<Vec<SessionState>, TlClientError> {
        self.with_reconnect(|c| async move {
            c.get_sessions_in_guild(tarpc::context::current(), guild_id)
                .await
        })
        .await
    }

    pub async fn report_guilds(
        &self,
        token: String,
        guilds: Vec<GuildId>,
    ) -> Result<(), TlClientError> {
        self.with_reconnect(|c| {
            let token = token.clone();
            let guilds = guilds.clone();
            async move {
                c.report_guilds(tarpc::context::current(), token, guilds)
                    .await
            }
        })
        .await
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
        let resp = self
            .with_reconnect(|c| {
                let req = req.clone();
                async move { c.execute(tarpc::context::current(), req).await }
            })
            .await?;
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
