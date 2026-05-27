use serde::{Deserialize, Serialize};

pub const SUBJECT_EMOJI_SCOPE_MATCH: &str = "emoji.scope_match";

/// Fire-and-forget message published by HQ when a custom Discord emoji is seen
/// in a message. The worker enumerates the four user-settings scopes that apply
/// to (user_id, guild_id) and, if any existing rule's image is nearly identical
/// to this new emoji, writes a new mapping rule in that same scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmojiScopeMatchRequest {
    pub emoji_id: String,
    pub emoji_name: String,
    pub emoji_animated: bool,
    pub guild_id: String,
    pub user_id: Option<String>,
}
