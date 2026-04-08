use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone)]
pub struct GuildInfo {
    pub id: u64,
    pub name: String,
    pub icon_url: Option<String>,
    pub can_manage: bool,
}

pub trait DiscordNameResolver: Send + Sync {
    fn guild_name(&self, guild_id: u64) -> Option<String>;
    fn channel_name(&self, guild_id: u64, channel_id: u64) -> Option<String>;
    fn guild_icon_url(&self, guild_id: u64) -> Option<String>;
    fn guilds_for_user(&self, discord_user_id: u64) -> Vec<GuildInfo>;
}

pub type DiscordNameResolverSlot = Arc<OnceLock<Arc<dyn DiscordNameResolver>>>;

pub fn make_resolver_slot() -> DiscordNameResolverSlot {
    Arc::new(OnceLock::new())
}
