use hq_core::service::DiscordNameResolver;
use poise::serenity_prelude::{Cache, ChannelId, GuildId};
use std::sync::Arc;

pub struct SerenityNameResolver {
    pub cache: Arc<Cache>,
}

impl DiscordNameResolver for SerenityNameResolver {
    fn guild_name(&self, guild_id: u64) -> Option<String> {
        self.cache.guild(GuildId::new(guild_id)).map(|g| g.name.clone())
    }

    fn channel_name(&self, guild_id: u64, channel_id: u64) -> Option<String> {
        let g = self.cache.guild(GuildId::new(guild_id))?;
        g.channels.get(&ChannelId::new(channel_id)).map(|c| c.name.clone())
    }
}
