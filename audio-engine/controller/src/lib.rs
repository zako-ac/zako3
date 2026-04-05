use std::sync::Arc;

use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::ErrorObjectOwned;
use tracing::instrument;

use zako3_audio_engine_core::engine::session_manager::SessionManager;
use zako3_audio_engine_core::error::ZakoError;
use zako3_audio_engine_core::types::{
    AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName, SessionState, TapName,
    TrackId, Volume, hq::DiscordUserId,
};

pub mod config;

fn to_rpc_error(e: ZakoError) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>)
}

#[rpc(server, client)]
pub trait AudioEngineRpc {
    #[method(name = "join")]
    async fn join(&self, guild_id: GuildId, channel_id: ChannelId) -> RpcResult<bool>;

    #[method(name = "leave")]
    async fn leave(&self, guild_id: GuildId) -> RpcResult<bool>;

    #[method(name = "play")]
    async fn play(
        &self,
        guild_id: GuildId,
        queue_name: QueueName,
        tap_name: TapName,
        audio_request_string: AudioRequestString,
        volume: Volume,
        discord_user_id: DiscordUserId,
    ) -> RpcResult<TrackId>;

    #[method(name = "set_volume")]
    async fn set_volume(
        &self,
        guild_id: GuildId,
        track_id: TrackId,
        volume: Volume,
    ) -> RpcResult<bool>;

    #[method(name = "stop")]
    async fn stop(&self, guild_id: GuildId, track_id: TrackId) -> RpcResult<bool>;

    #[method(name = "stop_many")]
    async fn stop_many(&self, guild_id: GuildId, filter: AudioStopFilter) -> RpcResult<bool>;

    #[method(name = "next_music")]
    async fn next_music(&self, guild_id: GuildId) -> RpcResult<bool>;

    #[method(name = "get_session_state")]
    async fn get_session_state(&self, guild_id: GuildId) -> RpcResult<SessionState>;
}

pub struct AudioEngineServer {
    pub session_manager: Arc<SessionManager>,
}

impl AudioEngineServer {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }
}

#[jsonrpsee::core::async_trait]
impl AudioEngineRpcServer for AudioEngineServer {
    #[instrument(skip(self))]
    async fn join(&self, guild_id: GuildId, channel_id: ChannelId) -> RpcResult<bool> {
        self.session_manager
            .join(guild_id, channel_id)
            .await
            .map_err(to_rpc_error)?;

        Ok(true)
    }

    #[instrument(skip(self))]
    async fn leave(&self, guild_id: GuildId) -> RpcResult<bool> {
        self.session_manager
            .leave(guild_id)
            .await
            .map_err(to_rpc_error)?;

        Ok(true)
    }

    #[instrument(skip(self))]
    async fn play(
        &self,
        guild_id: GuildId,
        queue_name: QueueName,
        tap_name: TapName,
        audio_request_string: AudioRequestString,
        volume: Volume,
        discord_user_id: DiscordUserId,
    ) -> RpcResult<TrackId> {
        let session = self
            .session_manager
            .get_session(guild_id)
            .ok_or_else(|| ErrorObjectOwned::owned(-32001, "Session not found", None::<()>))?;

        let track_id = session
            .play(
                queue_name,
                tap_name,
                audio_request_string,
                volume,
                discord_user_id,
            )
            .await
            .map_err(to_rpc_error)?;

        Ok(track_id)
    }

    #[instrument(skip(self))]
    async fn set_volume(
        &self,
        guild_id: GuildId,
        track_id: TrackId,
        volume: Volume,
    ) -> RpcResult<bool> {
        let session = self
            .session_manager
            .get_session(guild_id)
            .ok_or_else(|| ErrorObjectOwned::owned(-32001, "Session not found", None::<()>))?;

        session
            .set_volume(track_id, volume)
            .await
            .map_err(to_rpc_error)?;

        Ok(true)
    }

    #[instrument(skip(self))]
    async fn stop(&self, guild_id: GuildId, track_id: TrackId) -> RpcResult<bool> {
        let session = self
            .session_manager
            .get_session(guild_id)
            .ok_or_else(|| ErrorObjectOwned::owned(-32001, "Session not found", None::<()>))?;

        session.stop(track_id).await.map_err(to_rpc_error)?;

        Ok(true)
    }

    #[instrument(skip(self))]
    async fn stop_many(&self, guild_id: GuildId, filter: AudioStopFilter) -> RpcResult<bool> {
        let session = self
            .session_manager
            .get_session(guild_id)
            .ok_or_else(|| ErrorObjectOwned::owned(-32001, "Session not found", None::<()>))?;

        session.stop_many(filter).await.map_err(to_rpc_error)?;

        Ok(true)
    }

    #[instrument(skip(self))]
    async fn next_music(&self, guild_id: GuildId) -> RpcResult<bool> {
        let session = self
            .session_manager
            .get_session(guild_id)
            .ok_or_else(|| ErrorObjectOwned::owned(-32001, "Session not found", None::<()>))?;

        session.next_music().await.map_err(to_rpc_error)?;

        Ok(true)
    }

    #[instrument(skip(self))]
    async fn get_session_state(&self, guild_id: GuildId) -> RpcResult<SessionState> {
        let session = self
            .session_manager
            .get_session(guild_id)
            .ok_or_else(|| ErrorObjectOwned::owned(-32001, "Session not found", None::<()>))?;

        let state = session
            .session_state()
            .await
            .map_err(to_rpc_error)?
            .ok_or_else(|| {
                ErrorObjectOwned::owned(-32002, "Session state not found", None::<()>)
            })?;

        Ok(state)
    }
}
