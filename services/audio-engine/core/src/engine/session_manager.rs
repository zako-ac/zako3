use std::sync::Arc;

use dashmap::DashMap;
use tracing::instrument;
use zako3_audio_engine_audio::{create_opus_ringbuf_pair, metrics};

use crate::{
    audio::{PcmDecoder, create_thread_mixer},
    error::ZakoResult,
    service::{ArcDiscordService, ArcStateService, ArcTapHubService},
    session::{SessionControl, create_session_control},
    types::{ChannelId, GuildId, SessionState},
};

pub struct SessionManager {
    discord_service: ArcDiscordService,
    state_service: ArcStateService,
    taphub_service: ArcTapHubService,

    sessions: DashMap<(GuildId, ChannelId), Arc<SessionControl>>,
}

impl SessionManager {
    pub fn new(
        discord_service: ArcDiscordService,
        state_service: ArcStateService,
        taphub_service: ArcTapHubService,
    ) -> Self {
        SessionManager {
            discord_service,
            state_service,
            taphub_service,
            sessions: DashMap::new(),
        }
    }

    #[instrument(skip(self), fields(guild_id = %guild_id, channel_id = %channel_id))]
    async fn initiate_session(&self, guild_id: GuildId, channel_id: ChannelId) -> ZakoResult<()> {
        tracing::debug!("Initiating audio session");

        let (prod, cons) = create_opus_ringbuf_pair();

        let mixer = create_thread_mixer(prod);
        let decoder = PcmDecoder::new();

        let control = create_session_control(
            guild_id,
            channel_id,
            Arc::new(mixer),
            Arc::new(decoder),
            self.state_service.clone(),
            self.taphub_service.clone(),
        );

        self.discord_service.play_audio(guild_id, cons).await?;

        self.sessions.insert((guild_id, channel_id), control);

        metrics::inc_session_active();
        tracing::info!("Audio session initiated");

        Ok(())
    }

    #[instrument(skip(self), fields(guild_id = %guild_id, channel_id = %channel_id))]
    pub async fn join(&self, guild_id: GuildId, channel_id: ChannelId) -> ZakoResult<()> {
        tracing::info!("Joining voice channel");

        self.discord_service
            .join_voice_channel(guild_id, channel_id)
            .await?;

        let session = SessionState {
            guild_id,
            channel_id,
            queues: Default::default(),
        };

        self.initiate_session(guild_id, channel_id).await?;

        self.state_service.save_session(&session).await?;

        Ok(())
    }

    #[instrument(skip(self, session), fields(guild_id = %session.guild_id, channel_id = %session.channel_id))]
    pub async fn rejoin(&self, session: &SessionState) -> ZakoResult<()> {
        tracing::info!("Rejoining voice channel");

        self.discord_service
            .join_voice_channel(session.guild_id, session.channel_id)
            .await?;

        self.initiate_session(session.guild_id, session.channel_id)
            .await?;
        self.state_service.save_session(session).await?;

        Ok(())
    }

    pub async fn list_sessions(&self) -> ZakoResult<Vec<SessionState>> {
        self.state_service.list_sessions().await
    }

    pub async fn get_sessions_in_guild(&self, guild_id: GuildId) -> ZakoResult<Vec<SessionState>> {
        self.state_service.list_sessions_in_guild(guild_id).await
    }

    #[instrument(skip(self), fields(guild_id = %guild_id, channel_id = %channel_id))]
    pub async fn leave(&self, guild_id: GuildId, channel_id: ChannelId) -> ZakoResult<()> {
        tracing::info!("Leaving voice channel");

        self.discord_service.leave_voice_channel(guild_id).await?;
        self.state_service
            .delete_session(guild_id, channel_id)
            .await?;

        if self.sessions.remove(&(guild_id, channel_id)).is_some() {
            metrics::dec_session_active();
            tracing::info!("Audio session terminated");
        }

        Ok(())
    }

    /// Clean up a session that was terminated externally (e.g. bot kicked by admin).
    /// Does NOT call Discord — the bot is already disconnected.
    #[instrument(skip(self), fields(guild_id = %guild_id, channel_id = %channel_id))]
    pub async fn cleanup_session(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> ZakoResult<()> {
        tracing::info!("Cleaning up externally-disconnected session");

        self.state_service
            .delete_session(guild_id, channel_id)
            .await?;

        if self.sessions.remove(&(guild_id, channel_id)).is_some() {
            metrics::dec_session_active();
            tracing::info!("Session cleaned up after external disconnect");
        }

        Ok(())
    }

    pub fn get_session(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> Option<Arc<SessionControl>> {
        self.sessions
            .get(&(guild_id, channel_id))
            .map(|s| s.clone())
    }

    pub fn get_sessions_in_guild_local(&self, guild_id: GuildId) -> Vec<Arc<SessionControl>> {
        self.sessions
            .iter()
            .filter(|entry| entry.key().0 == guild_id)
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub async fn fetch_discord_voice_state(&self) -> ZakoResult<Vec<(GuildId, ChannelId)>> {
        self.discord_service.get_active_voice_connections().await
    }
}
