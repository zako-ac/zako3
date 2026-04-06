use std::sync::{Arc, OnceLock};

pub trait DiscordNameResolver: Send + Sync {
    fn guild_name(&self, guild_id: u64) -> Option<String>;
    fn channel_name(&self, guild_id: u64, channel_id: u64) -> Option<String>;
}

pub type DiscordNameResolverSlot = Arc<OnceLock<Arc<dyn DiscordNameResolver>>>;

pub fn make_resolver_slot() -> DiscordNameResolverSlot {
    Arc::new(OnceLock::new())
}
