use serde::{Deserialize, Serialize};

use crate::hq::{DiscordUserId, TapId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum UseHistoryEntry {
    PlayAudio(PlayAudioHistory),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayAudioHistory {
    pub user_id: Option<UserId>,
    pub discord_user_id: Option<DiscordUserId>,
    pub ars_length: usize,
    pub trace_id: Option<String>,
    pub tap_id: TapId,
    pub cache_hit: bool,
    pub success: bool,
}
