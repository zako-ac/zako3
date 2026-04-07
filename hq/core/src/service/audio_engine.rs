use std::sync::Arc;

use hq_types::{
    hq::DiscordUserId, AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName,
    SessionState, TapName, TrackId, Volume,
};
use zako3_audio_engine_client::client::AudioEngineRpcClient;

use crate::{CoreError, CoreResult};

#[derive(Clone)]
pub struct AudioEngineService {
    client: Arc<AudioEngineRpcClient>,
}

impl AudioEngineService {
    pub fn new(client: Arc<AudioEngineRpcClient>) -> Self {
        Self { client }
    }

    pub async fn join(&self, guild_id: GuildId, channel_id: ChannelId) -> CoreResult<bool> {
        self.client
            .join(guild_id, channel_id)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }

    pub async fn leave(&self, guild_id: GuildId, channel_id: ChannelId) -> CoreResult<bool> {
        self.client
            .leave(guild_id, channel_id)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }

    pub async fn play(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        queue_name: QueueName,
        tap_name: TapName,
        audio_request_string: AudioRequestString,
        volume: Volume,
        discord_user_id: DiscordUserId,
    ) -> CoreResult<TrackId> {
        self.client
            .play(
                guild_id,
                channel_id,
                queue_name,
                tap_name,
                audio_request_string,
                volume,
                discord_user_id,
            )
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }

    pub async fn set_volume(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        track_id: TrackId,
        volume: Volume,
    ) -> CoreResult<bool> {
        self.client
            .set_volume(guild_id, channel_id, track_id, volume)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }

    pub async fn stop(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        track_id: TrackId,
    ) -> CoreResult<bool> {
        self.client
            .stop(guild_id, channel_id, track_id)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }

    pub async fn stop_many(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        filter: AudioStopFilter,
    ) -> CoreResult<bool> {
        self.client
            .stop_many(guild_id, channel_id, filter)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }

    pub async fn next_music(&self, guild_id: GuildId, channel_id: ChannelId) -> CoreResult<bool> {
        self.client
            .next_music(guild_id, channel_id)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }

    pub async fn pause(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        track_id: TrackId,
    ) -> CoreResult<bool> {
        self.client
            .pause(guild_id, channel_id, track_id)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }

    pub async fn resume(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        track_id: TrackId,
    ) -> CoreResult<bool> {
        self.client
            .resume(guild_id, channel_id, track_id)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }

    pub async fn get_session_state(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> CoreResult<SessionState> {
        self.client
            .get_session_state(guild_id, channel_id)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }

    pub async fn get_sessions_in_guild(&self, guild_id: GuildId) -> CoreResult<Vec<SessionState>> {
        self.client
            .get_sessions_in_guild(guild_id)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }
}
