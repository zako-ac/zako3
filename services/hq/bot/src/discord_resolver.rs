use hq_core::service::{DiscordNameResolver, DiscordUserInfo, GuildInfo};
use poise::serenity_prelude::{Cache, ChannelId, GuildId, Http, UserId};
use std::sync::Arc;

pub struct SerenityNameResolver {
    pub cache: Arc<Cache>,
    pub http: Arc<Http>,
}

impl DiscordNameResolver for SerenityNameResolver {
    fn guild_name(&self, guild_id: u64) -> Option<String> {
        let gid = GuildId::new(guild_id);
        if let Some(name) = self.cache.guild(gid).map(|g| g.name.clone()) {
            return Some(name);
        }
        let http = self.http.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                http.get_guild(gid).await.ok().map(|g| g.name)
            })
        })
    }

    fn channel_name(&self, guild_id: u64, channel_id: u64) -> Option<String> {
        let gid = GuildId::new(guild_id);
        let cid = ChannelId::new(channel_id);
        if let Some(name) = self
            .cache
            .guild(gid)
            .and_then(|g| g.channels.get(&cid).map(|c| c.name.clone()))
        {
            return Some(name);
        }
        let http = self.http.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                http.get_channel(cid).await.ok().and_then(|c| c.guild()).map(|gc| gc.name)
            })
        })
    }

    fn guild_icon_url(&self, guild_id: u64) -> Option<String> {
        let gid = GuildId::new(guild_id);
        if let Some(url) = self.cache.guild(gid).and_then(|g| g.icon_url()) {
            return Some(url);
        }
        let http = self.http.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                http.get_guild(gid).await.ok().and_then(|g| g.icon_url())
            })
        })
    }

    fn user_info(&self, user_id: u64) -> Option<DiscordUserInfo> {
        let uid = UserId::new(user_id);
        if let Some(user) = self.cache.user(uid) {
            return Some(DiscordUserInfo {
                id: user_id,
                name: user.name.clone(),
                avatar_url: Some(user.face()),
                global_name: user.global_name.clone(),
            });
        }
        let http = self.http.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                http.get_user(uid).await.ok().map(|user| {
                    let avatar_url = Some(user.face());
                    DiscordUserInfo {
                        id: user_id,
                        name: user.name,
                        avatar_url,
                        global_name: user.global_name,
                    }
                })
            })
        })
    }

    #[tracing::instrument(skip(self), name = "discord_resolver.guilds_for_user")]
    fn guilds_for_user(&self, discord_user_id: u64) -> Vec<GuildInfo> {
        let user_id = UserId::new(discord_user_id);
        let all_guild_ids: Vec<_> = self.cache.guilds().into_iter().collect();
        let total_bot_guilds = all_guild_ids.len();
        tracing::debug!(total_bot_guilds, "Bot is in {} total guilds", total_bot_guilds);

        let mut result: Vec<GuildInfo> = Vec::new();
        let mut need_http: Vec<GuildId> = Vec::new();

        for guild_id in &all_guild_ids {
            if let Some(guild) = self.cache.guild(*guild_id) {
                if let Some(member) = guild.members.get(&user_id) {
                    let permissions = guild.member_permissions(member);
                    let can_manage = permissions.manage_guild();
                    tracing::debug!(
                        guild_id = %guild_id.get(),
                        guild_name = %guild.name,
                        member_count = guild.members.len(),
                        can_manage,
                        "User is member of guild"
                    );
                    result.push(GuildInfo {
                        id: guild_id.get(),
                        name: guild.name.clone(),
                        icon_url: guild.icon_url(),
                        can_manage,
                    });
                } else {
                    need_http.push(*guild_id);
                }
            }
        }

        if !need_http.is_empty() {
            let http = self.http.clone();
            let cache = self.cache.clone();
            let http_results = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    let mut fetched = Vec::new();
                    for guild_id in need_http {
                        match http.get_member(guild_id, user_id).await {
                            Ok(member) => {
                                if let Some(guild) = cache.guild(guild_id) {
                                    let permissions = guild.member_permissions(&member);
                                    let can_manage = permissions.manage_guild();
                                    tracing::debug!(
                                        guild_id = %guild_id.get(),
                                        guild_name = %guild.name,
                                        can_manage,
                                        "User is member of guild (http fallback)"
                                    );
                                    fetched.push(GuildInfo {
                                        id: guild_id.get(),
                                        name: guild.name.clone(),
                                        icon_url: guild.icon_url(),
                                        can_manage,
                                    });
                                }
                            }
                            Err(_) => {}
                        }
                    }
                    fetched
                })
            });
            result.extend(http_results);
        }

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
        let gid = GuildId::new(guild_id);
        let uid = UserId::new(user_id);
        if let Some(guild) = self.cache.guild(gid) {
            if let Some(member) = guild.members.get(&uid) {
                return member.nick.clone();
            }
        }
        let http = self.http.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                http.get_member(gid, uid).await.ok().and_then(|m| m.nick)
            })
        })
    }

    fn bot_guilds(&self) -> Vec<u64> {
        let cached: Vec<u64> = self.cache.guilds().into_iter().map(|gid| gid.get()).collect();
        if !cached.is_empty() {
            return cached;
        }
        let http = self.http.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                http.get_guilds(None, None)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|g| g.id.get())
                    .collect()
            })
        })
    }
}
