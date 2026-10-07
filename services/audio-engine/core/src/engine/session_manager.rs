use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::Mutex as AsyncMutex;
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

    /// Serialises the voice-connection work for one guild. A bot token gets a
    /// single voice connection per guild, so joining, moving and leaving are
    /// all operations on the same connection and must not overlap. See `join`.
    voice_ops: DashMap<GuildId, Arc<AsyncMutex<()>>>,
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
            voice_ops: DashMap::new(),
        }
    }

    /// The lock that every voice-connection operation in `guild_id` holds.
    fn guild_voice_lock(&self, guild_id: GuildId) -> Arc<AsyncMutex<()>> {
        let entry = self
            .voice_ops
            .entry(guild_id)
            .or_insert_with(|| Arc::new(AsyncMutex::new(())));
        Arc::clone(&entry)
    }

    /// Whether songbird already holds a connection to this channel.
    async fn is_connected_to(&self, guild_id: GuildId, channel_id: ChannelId) -> bool {
        self.discord_service
            .get_active_voice_connections()
            .await
            .map(|connections| connections.contains(&(guild_id, channel_id)))
            .unwrap_or(false)
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

        // One voice-connection operation at a time per guild. A bot token gets
        // one voice connection per guild, so a join for one channel and a join
        // for another are operations on the same connection; letting them race
        // in songbird is what left a bot connected at Discord's side with no
        // session — neither attempt finished, and HQ could then only "repair"
        // that by letting a *second* bot into the room.
        let join_lock = self.guild_voice_lock(guild_id);
        let _guard = join_lock.lock_owned().await;

        // If the bot is already connected to the channel there is nothing to
        // connect — and asking songbird to join again is actively harmful: a
        // second join on a connection that is still coming up makes it abandon
        // that one and start over, which is how the bot ended up connected with
        // no session recorded. Keep the connection and (re)create the session.
        if self.is_connected_to(guild_id, channel_id).await {
            tracing::info!("Already connected to this channel; re-establishing the session");
        } else {
            self.discord_service
                .join_voice_channel(guild_id, channel_id)
                .await?;
        }

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

        let join_lock = self.guild_voice_lock(session.guild_id);
        let _guard = join_lock.lock_owned().await;

        if self
            .is_connected_to(session.guild_id, session.channel_id)
            .await
        {
            tracing::info!("Already connected to this channel; re-establishing the session");
        } else {
            self.discord_service
                .join_voice_channel(session.guild_id, session.channel_id)
                .await?;
        }

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

        // Same lock as joining: a leave that lands while a join is still coming
        // up tears the connection out from under it, and the session the join
        // then records belongs to a connection that no longer exists.
        let voice_lock = self.guild_voice_lock(guild_id);
        let _guard = voice_lock.lock_owned().await;

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
