use std::collections::HashSet;
use std::sync::Arc;

use hq_core::service::VoicePresence;
use poise::serenity_prelude as serenity;

/// Discord's own voice state, read straight out of the master bot's serenity
/// cache.
///
/// The cache is maintained by the gateway: Discord tells us about every voice
/// state change in every guild the master shares, including changes to *other*
/// bots such as our worker bots. That makes it the authority on physical
/// presence, and it is why HQ no longer needs a private "who did we ask to join
/// where" registry that can drift out of sync with reality.
pub struct SerenityVoicePresence {
    cache: Arc<serenity::Cache>,
}

impl SerenityVoicePresence {
    pub fn new(cache: Arc<serenity::Cache>) -> Self {
        Self { cache }
    }
}

impl VoicePresence for SerenityVoicePresence {
    fn bot_voice_states(&self, guild_id: u64, bot_ids: &HashSet<u64>) -> Vec<(u64, u64)> {
        let Some(guild) = self.cache.guild(serenity::GuildId::new(guild_id)) else {
            return Vec::new();
        };

        guild
            .voice_states
            .values()
            .filter(|vs| bot_ids.contains(&vs.user_id.get()))
            .filter_map(|vs| {
                vs.channel_id
                    .map(|channel| (channel.get(), vs.user_id.get()))
            })
            .collect()
    }
}
