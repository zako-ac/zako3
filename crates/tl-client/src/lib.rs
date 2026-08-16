use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use jsonrpsee::core::client::Error as RpcClientError;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use opentelemetry::global;
use thiserror::Error;
use tl_protocol::{
    AudioEngineCommand, AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineError,
    AudioEngineSessionCommand, AudioPlayRequest, SessionInfo, TrafficLightRpcClient,
};
use tokio::sync::RwLock;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use zako3_types::{
    AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName, SessionState, TrackId,
    Volume,
    hq::{DiscordUserId, TapId},
};

#[derive(Debug, Error)]
pub enum TlClientError {
    #[error("Already in VC")]
    AlreadyJoined,
    #[error("Not in VC")]
    NotJoined,
    #[error("Permission denied")]
    PermissionDenied,
    #[error(transparent)]
    Tap(zako3_types::TapHubError),
    #[error("{0}")]
    Transport(anyhow::Error),
}

pub struct TlClient {
    url: Arc<str>,
    /// The jsonrpsee HttpClient keeps an internal keep-alive connection pool. A
    /// pooled connection can go stale when the TL server restarts: reusing it
    /// then fails permanently. We rebuild the client (fresh pool) on a
    /// transport-level failure and retry once — mirroring the taphub-stall-
    /// recovery pattern.
    client: Arc<RwLock<HttpClient>>,
}

impl TlClient {
    /// Connect to a TL instance at `url` (e.g. `"http://127.0.0.1:7070"`).
    pub async fn connect(url: &str) -> Result<Self> {
        let client =
            Self::build(url).with_context(|| format!("failed to build TL client for {url}"))?;
        Ok(Self {
            url: Arc::from(url),
            client: Arc::new(RwLock::new(client)),
        })
    }

    fn build(url: &str) -> Result<HttpClient> {
        // Fail fast instead of jsonrpsee's default 60s when a pooled connection
        // is wedged, so a dead TL is noticed quickly rather than stalling callers.
        HttpClientBuilder::default()
            .request_timeout(Duration::from_secs(10))
            .build(url)
            .map_err(Into::into)
    }

    fn map_err(e: RpcClientError) -> TlClientError {
        TlClientError::Transport(anyhow::anyhow!("{e}"))
    }

    /// Run one RPC round-trip. On a transport-level error (the telltale of a
    /// stale keep-alive pooled connection after a server restart) we rebuild the
    /// client — which resets the pool — and retry once. Application-level
    /// (JSON-RPC call) errors are returned as-is and never retried.
    async fn round_trip<T>(
        &self,
        op: impl for<'a> Fn(
            &'a HttpClient,
        )
            -> Pin<Box<dyn Future<Output = Result<T, RpcClientError>> + Send + 'a>>,
    ) -> Result<T, TlClientError> {
        // First attempt on the current pooled client.
        {
            let client = self.client.read().await;
            match op(&client).await {
                Ok(v) => return Ok(v),
                Err(e) if Self::is_transport(&e) => {
                    tracing::warn!(
                        "TL RPC transport error; rebuilding client and retrying once: {e}"
                    );
                }
                Err(e) => return Err(Self::map_err(e)),
            }
        }

        // Rebuild a fresh client (fresh connection pool) and retry once.
        let fresh = match Self::build(&self.url) {
            Ok(c) => c,
            Err(e) => {
                return Err(TlClientError::Transport(anyhow::anyhow!(
                    "failed to rebuild TL client: {e:#}"
                )));
            }
        };
        *self.client.write().await = fresh;
        let client = self.client.read().await;
        op(&client).await.map_err(Self::map_err)
    }

    fn is_transport(e: &RpcClientError) -> bool {
        matches!(
            e,
            RpcClientError::Transport(_)
                | RpcClientError::RestartNeeded(_)
                | RpcClientError::RequestTimeout
        )
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
            AudioEngineCommandResponse::Error(AudioEngineError::Tap(t)) => {
                Err(TlClientError::Tap(t))
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

    pub async fn join(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> Result<(), TlClientError> {
        let req = Self::build_req(guild_id, channel_id, AudioEngineCommand::Join);
        let resp = self
            .round_trip(|c| Box::pin(c.execute(req.clone())))
            .await?;
        Self::ok_or_err(resp)
    }

    pub async fn leave(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> Result<(), TlClientError> {
        let req = Self::build_req(
            guild_id,
            channel_id,
            AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Leave),
        );
        let resp = self
            .round_trip(|c| Box::pin(c.execute(req.clone())))
            .await?;
        Self::ok_or_err(resp)
    }

    pub async fn play(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        queue_name: QueueName,
        tap_id: TapId,
        ars: AudioRequestString,
        volume: Volume,
        initiator: DiscordUserId,
    ) -> Result<(), TlClientError> {
        let req = Self::build_req(
            guild_id,
            channel_id,
            AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Play(AudioPlayRequest {
                queue_name,
                tap_id,
                ars,
                volume,
                initiator,
                headers: {
                    let mut h = HashMap::new();
                    let cx = tracing::Span::current().context();
                    global::get_text_map_propagator(|p| p.inject_context(&cx, &mut h));
                    h
                },
            })),
        );
        let resp = self
            .round_trip(|c| Box::pin(c.execute(req.clone())))
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
            .round_trip(|c| Box::pin(c.execute(req.clone())))
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
            .round_trip(|c| Box::pin(c.execute(req.clone())))
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
            .round_trip(|c| Box::pin(c.execute(req.clone())))
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
            .round_trip(|c| Box::pin(c.execute(req.clone())))
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
            .round_trip(|c| Box::pin(c.execute(req.clone())))
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
            .round_trip(|c| Box::pin(c.execute(req.clone())))
            .await?;
        Self::ok_or_err(resp)
    }

    pub async fn get_sessions_in_guild(
        &self,
        guild_id: GuildId,
    ) -> Result<Vec<SessionState>, TlClientError> {
        self.round_trip(|c| Box::pin(c.get_sessions_in_guild(guild_id)))
            .await
    }

    pub async fn list_bot_ids(&self) -> Result<Vec<String>, TlClientError> {
        self.round_trip(|c| Box::pin(c.list_bot_ids())).await
    }

    pub async fn report_guilds(
        &self,
        token: String,
        guilds: Vec<GuildId>,
    ) -> Result<(), TlClientError> {
        self.round_trip(|c| Box::pin(c.report_guilds(token.clone(), guilds.clone())))
            .await
    }

    pub async fn register_ae(&self, listen_addr: String) -> Result<String, TlClientError> {
        self.round_trip(|c| Box::pin(c.register_ae(listen_addr.clone())))
            .await
    }

    pub async fn heartbeat_ae(
        &self,
        token: String,
        listen_addr: String,
    ) -> Result<(), TlClientError> {
        self.round_trip(|c| Box::pin(c.heartbeat_ae(token.clone(), listen_addr.clone())))
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
            .round_trip(|c| Box::pin(c.execute(req.clone())))
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
            AudioEngineCommandResponse::Error(AudioEngineError::Tap(t)) => {
                Err(TlClientError::Tap(t))
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
