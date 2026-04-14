use std::sync::{Arc, OnceLock};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildInfo {
    pub id: u64,
    pub name: String,
    pub icon_url: Option<String>,
    pub can_manage: bool,
}

#[derive(Debug, Clone)]
pub struct DiscordUserInfo {
    pub id: u64,
    pub name: String,
    pub avatar_url: Option<String>,
    pub global_name: Option<String>,
}

pub trait DiscordNameResolver: Send + Sync {
    fn guild_name(&self, guild_id: u64) -> Option<String>;
    fn channel_name(&self, guild_id: u64, channel_id: u64) -> Option<String>;
    fn guild_icon_url(&self, guild_id: u64) -> Option<String>;
    fn guilds_for_user(&self, discord_user_id: u64) -> Vec<GuildInfo>;
    fn user_info(&self, user_id: u64) -> Option<DiscordUserInfo>;
    fn guild_nickname(&self, guild_id: u64, user_id: u64) -> Option<String>;
    fn bot_guilds(&self) -> Vec<u64>;
}

pub type DiscordNameResolverSlot = Arc<OnceLock<Arc<dyn DiscordNameResolver>>>;

pub fn make_resolver_slot() -> DiscordNameResolverSlot {
    Arc::new(OnceLock::new())
}
