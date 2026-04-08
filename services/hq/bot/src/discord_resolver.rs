use hq_core::service::{DiscordNameResolver, GuildInfo};
use poise::serenity_prelude::{Cache, ChannelId, GuildId, UserId};
use std::sync::Arc;

pub struct SerenityNameResolver {
    pub cache: Arc<Cache>,
}

impl DiscordNameResolver for SerenityNameResolver {
    fn guild_name(&self, guild_id: u64) -> Option<String> {
        self.cache
            .guild(GuildId::new(guild_id))
            .map(|g| g.name.clone())
    }

    fn channel_name(&self, guild_id: u64, channel_id: u64) -> Option<String> {
        let g = self.cache.guild(GuildId::new(guild_id))?;
        g.channels
            .get(&ChannelId::new(channel_id))
            .map(|c| c.name.clone())
    }

    fn guild_icon_url(&self, guild_id: u64) -> Option<String> {
        self.cache
            .guild(GuildId::new(guild_id))
            .and_then(|g| g.icon_url())
    }

    fn guilds_for_user(&self, discord_user_id: u64) -> Vec<GuildInfo> {
        let user_id = UserId::new(discord_user_id);
        self.cache
            .guilds()
            .into_iter()
            .filter_map(|guild_id| {
                let guild = self.cache.guild(guild_id)?;
                let member = guild.members.get(&user_id)?;
                let permissions = guild.member_permissions(member);
                Some(GuildInfo {
                    id: guild_id.get(),
                    name: guild.name.clone(),
                    icon_url: guild.icon_url(),
                    can_manage: permissions.manage_guild(),
                })
            })
            .collect()
    }
}
