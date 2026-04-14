use hq_core::service::{DiscordNameResolver, DiscordUserInfo, GuildInfo};
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

    fn user_info(&self, user_id: u64) -> Option<DiscordUserInfo> {
        let user = self.cache.user(UserId::new(user_id))?;
        Some(DiscordUserInfo {
            id: user_id,
            name: user.name.clone(),
            avatar_url: Some(user.face()),
            global_name: user.global_name.clone(),
        })
    }

    #[tracing::instrument(skip(self), name = "discord_resolver.guilds_for_user")]
    fn guilds_for_user(&self, discord_user_id: u64) -> Vec<GuildInfo> {
        let user_id = UserId::new(discord_user_id);
        let all_guild_ids: Vec<_> = self.cache.guilds().into_iter().collect();
        let total_bot_guilds = all_guild_ids.len();
        tracing::debug!(total_bot_guilds, "Bot is in {} total guilds", total_bot_guilds);

        let result: Vec<GuildInfo> = all_guild_ids
            .into_iter()
            .filter_map(|guild_id| {
                let guild = self.cache.guild(guild_id)?;
                let member = guild.members.get(&user_id)?;
                let permissions = guild.member_permissions(member);
                let can_manage = permissions.manage_guild();

                tracing::debug!(
                    guild_id = %guild_id.get(),
                    guild_name = %guild.name,
                    member_count = guild.members.len(),
                    can_manage,
                    "User is member of guild"
                );

                Some(GuildInfo {
                    id: guild_id.get(),
                    name: guild.name.clone(),
                    icon_url: guild.icon_url(),
                    can_manage,
                })
            })
            .collect();

        tracing::info!(
            found_user_in = result.len(),
            total_bot_guilds,
            "User found in {} guilds (out of {} bot guilds)",
            result.len(),
            total_bot_guilds
        );
        result
    }

    fn guild_nickname(&self, guild_id: u64, user_id: u64) -> Option<String> {
        self.cache
            .guild(GuildId::new(guild_id))?
            .members
            .get(&UserId::new(user_id))?
            .nick
            .clone()
    }

    fn bot_guilds(&self) -> Vec<u64> {
        self.cache
            .guilds()
            .into_iter()
            .map(|gid| gid.get())
            .collect()
    }
}
