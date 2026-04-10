use std::sync::Arc;

use parking_lot::RwLock;
use rustc_hash::FxHashSet;
use zako3_types::GuildId;

#[derive(Clone, Debug)]
pub struct WorkerPermissions {
    allowed_guilds: Arc<RwLock<FxHashSet<GuildId>>>,
}

impl WorkerPermissions {
    pub fn new() -> Self {
        Self {
            allowed_guilds: Arc::new(RwLock::new(FxHashSet::default())),
        }
    }

    pub fn set_allowed_guilds(&self, set: Vec<GuildId>) {
        let mut allowed_guilds = self.allowed_guilds.write();
        allowed_guilds.clear();
        allowed_guilds.extend(set);
    }

    pub fn add_allowed_guild(&self, guild_id: GuildId) {
        self.allowed_guilds.write().insert(guild_id);
    }

    pub fn remove_allowed_guild(&self, guild_id: &GuildId) {
        self.allowed_guilds.write().remove(guild_id);
    }

    pub fn is_guild_allowed(&self, guild_id: &GuildId) -> bool {
        self.allowed_guilds.read().contains(guild_id)
    }
}
