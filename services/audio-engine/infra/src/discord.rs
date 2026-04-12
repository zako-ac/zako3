use std::num::NonZeroU64;
use std::sync::Arc;

use async_trait::async_trait;
use serenity::all::ChannelId as SerenityChannelId;
use serenity::cache::Cache;
use songbird::Songbird;
use tracing::instrument;
use zako3_audio_engine_audio::OpusCons;
use zako3_audio_engine_core::{
    error::ZakoResult,
    service::discord::DiscordService,
    types::{ChannelId, GuildId},
};

pub struct SongbirdDiscordService {
    manager: Arc<Songbird>,
    cache: Arc<Cache>,
}

impl SongbirdDiscordService {
    pub fn new(manager: Arc<Songbird>, cache: Arc<Cache>) -> Self {
        Self { manager, cache }
    }

    fn to_songbird_guild_id(guild_id: GuildId) -> songbird::id::GuildId {
        // TODO remove unwrap
        songbird::id::GuildId::from(NonZeroU64::new(guild_id.into()).unwrap())
    }

    fn to_serenity_channel_id(channel_id: ChannelId) -> SerenityChannelId {
        SerenityChannelId::new(channel_id.into())
    }
}

#[async_trait]
impl DiscordService for SongbirdDiscordService {
    async fn get_active_voice_connections(&self) -> ZakoResult<Vec<(GuildId, ChannelId)>> {
        let mut result = Vec::new();

        // Iterate over all guilds in the cache
        for guild_id in self.cache.guilds() {
            let g_id = Self::to_songbird_guild_id(GuildId::from(guild_id.get()));

            // Check if songbird has an active connection in this guild
            if let Some(call_lock) = self.manager.get(g_id) {
                let call = call_lock.lock().await;

                // Get current connection info
                if let Some(conn) = call.current_connection() {
                    if let Some(songbird_channel_id) = conn.channel_id {
                        // Convert songbird ChannelId to zako3 ChannelId
                        let channel_id = ChannelId::from(songbird_channel_id.0.get());
                        let guild_id = GuildId::from(guild_id.get());
                        result.push((guild_id, channel_id));
                    }
                }
            }
        }

        Ok(result)
    }

    async fn join_voice_channel(&self, guild_id: GuildId, channel_id: ChannelId) -> ZakoResult<()> {
        let g_id = Self::to_songbird_guild_id(guild_id);
        let c_id = Self::to_serenity_channel_id(channel_id);

        let _handler = self.manager.join(g_id, c_id).await?;
        Ok(())
    }

    async fn leave_voice_channel(&self, guild_id: GuildId) -> ZakoResult<()> {
        let g_id = Self::to_songbird_guild_id(guild_id);
        self.manager.remove(g_id).await?;
        Ok(())
    }

    #[instrument(skip(self, stream))]
    async fn play_audio(&self, guild_id: GuildId, stream: OpusCons) -> ZakoResult<()> {
        tracing::info!("Playing audio in guild {:?}", guild_id);

        let g_id = Self::to_songbird_guild_id(guild_id);

        if let Some(call_lock) = self.manager.get(g_id) {
            let call = call_lock.lock().await;

            // Play direct opus stream
            call.play_direct_opus(stream);
        }

        Ok(())
    }
}
