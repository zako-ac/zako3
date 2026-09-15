use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

/// HQ's read-only view of Discord's own voice state, as seen by the master
/// bot's serenity cache.
///
/// This — not a private registry of "who we asked to join where" — is the
/// authority for which bot is in which channel. Discord is the only party that
/// actually knows whether a voice connection is up, so deriving membership from
/// its gateway state removes the whole class of bugs where an event-driven
/// cache drifts away from reality.
///
/// It lives in `hq-bot`, which owns the serenity client, and is installed into
/// the slot the same way the Discord name resolver is. `hq-core` only ever
/// reads through the trait; when the slot is still empty (the bot has not
/// finished starting) every query answers "nothing known", which callers treat
/// as "no session".
pub trait VoicePresence: Send + Sync {
    /// Every `(channel_id, bot_user_id)` pair in `guild_id` where the user is
    /// one of `bot_ids` and is currently connected to a voice channel.
    fn bot_voice_states(&self, guild_id: u64, bot_ids: &HashSet<u64>) -> Vec<(u64, u64)>;
}

pub type VoicePresenceSlot = Arc<OnceLock<Arc<dyn VoicePresence>>>;

pub fn make_voice_presence_slot() -> VoicePresenceSlot {
    Arc::new(OnceLock::new())
}
