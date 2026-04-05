use serde::{Deserialize, Serialize};

use super::TapId;

#[derive(Debug, Clone, Serialize, Deserialize, zod_gen_derive::ZodSchema)]
pub struct TextMappingRule {
    pub pattern: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, zod_gen_derive::ZodSchema)]
pub struct EmojiMappingRule {
    pub emoji_id: String,
    pub emoji_name: String,
    pub emoji_image_url: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, zod_gen_derive::ZodSchema)]

#[serde(rename_all = "snake_case")]
pub enum TextReadingRule {
    Always,
    InVoiceChannel,
    OnMicMute,
}

#[derive(Debug, Clone, Serialize, Deserialize, zod_gen_derive::ZodSchema)]

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

#[derive(Debug, Clone, Serialize, Deserialize, zod_gen_derive::ZodSchema)]
pub struct Settings {
    pub text_mappings: Vec<TextMappingRule>,
    pub emoji_mappings: Vec<EmojiMappingRule>,
    pub text_reading_rule: TextReadingRule,
    pub user_join_leave_alert: UserJoinLeaveAlert,
    pub max_message_length: u16,
    pub enable_tts_queue: bool,
    pub tts_voice: Option<TapId>,
}
