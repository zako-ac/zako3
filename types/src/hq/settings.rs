use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::TapId;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub struct TextMappingRule {
    pub pattern: String,
    pub replacement: String,
    pub case_sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub struct EmojiMappingRule {
    pub emoji_id: String,
    pub emoji_name: String,
    pub emoji_image_url: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextReadingRule {
    Always,
    InVoiceChannel,
    OnMicMute,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum UserJoinLeaveAlert {
    Auto,
    Off,

    /// XXX 등장, XXX 퇴장
    WithDifferentUsername(String),

    Custom {
        join_message: String,
        leave_message: String,
    },
}

/// A settings field at a specific scope. `None` means "not configured at this scope".
/// `Important` reverses cascade priority — less-specific scopes with `Important` beat
/// more-specific scopes with `Normal`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum UserSettingsField<T> {
    None,
    Normal(T),
    Important(T),
}

/// Partial (per-scope) settings. Each field is `None` if not configured at this scope.
/// Use `PartialUserSettings::fold` to merge two scopes, and `resolve` to get the
/// final concrete `UserSettings`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub struct PartialUserSettings {
    pub text_mappings: UserSettingsField<Vec<TextMappingRule>>,
    pub emoji_mappings: UserSettingsField<Vec<EmojiMappingRule>>,
    pub text_reading_rule: UserSettingsField<TextReadingRule>,
    pub user_join_leave_alert: UserSettingsField<UserJoinLeaveAlert>,
    pub max_message_length: UserSettingsField<u16>,
    pub enable_tts_queue: UserSettingsField<bool>,
    pub tts_voice: UserSettingsField<Option<TapId>>,
}

impl PartialUserSettings {
    pub fn empty() -> Self {
        Self {
            text_mappings: UserSettingsField::None,
            emoji_mappings: UserSettingsField::None,
            text_reading_rule: UserSettingsField::None,
            user_join_leave_alert: UserSettingsField::None,
            max_message_length: UserSettingsField::None,
            enable_tts_queue: UserSettingsField::None,
            tts_voice: UserSettingsField::None,
        }
    }

    /// Merge two settings layers.
    ///
    /// Rules per field:
    /// - `less_important` has `Important(y)` → result is `Important(y)` (admin override wins)
    /// - `more_important` is `None` → take `less_important` value
    /// - otherwise → take `more_important` value
    ///
    /// Full cascade: `fold(guild_user, fold(user, fold(guild, global)))`
    pub fn fold(more_important: &Self, less_important: &Self) -> Self {
        todo!()
    }

    /// Apply hardcoded defaults for any remaining `None` fields, producing
    /// the concrete `UserSettings` used by TTS/audio.
    pub fn resolve(self) -> UserSettings {
        todo!()
    }
}

/// Resolved (concrete) settings — all fields are present. Produced by
/// `PartialUserSettings::resolve` after cascading all scopes.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub struct UserSettings {
    pub text_mappings: Vec<TextMappingRule>,
    pub emoji_mappings: Vec<EmojiMappingRule>,
    pub text_reading_rule: TextReadingRule,
    pub user_join_leave_alert: UserJoinLeaveAlert,
    pub max_message_length: u16,
    pub enable_tts_queue: bool,
    pub tts_voice: Option<TapId>,
}
