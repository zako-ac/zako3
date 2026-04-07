use serde::{Deserialize, Serialize};

/// Input to a WASM mapper — the data the host provides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Input {
    /// The TTS text to process
    pub text: String,
    /// Text and emoji mapping rules (if mapper declared MappingList)
    #[serde(default)]
    pub mapping_list: Option<MappingList>,
    /// Discord user ID of the TTS caller (if mapper declared CallerInfo)
    #[serde(default)]
    pub caller_info: Option<CallerInfo>,
    /// History of mappers that ran before + list of mappers to run after (if mapper declared MapperList)
    #[serde(default)]
    pub mapper_list: Option<MapperList>,
    /// Guild ID where the TTS will be played (if mapper declared DiscordInfo)
    #[serde(default)]
    pub guild_id: Option<u64>,
    /// Channel ID where the TTS will be played (if mapper declared DiscordInfo)
    #[serde(default)]
    pub channel_id: Option<u64>,
}

impl Input {
    /// Query Discord channel info by channel_id from this input
    /// Returns None if DiscordInfo was not declared in mapper or channel lookup failed
    pub fn query_channel(&self) -> Option<ChannelInfo> {
        self.channel_id
            .and_then(|id| crate::host::query_channel_raw(&id.to_string()))
    }

    /// Query Discord user info by Discord user ID
    /// Returns None if DiscordInfo was not declared in mapper or user lookup failed
    pub fn query_user(&self, discord_user_id: &str) -> Option<UserInfo> {
        crate::host::query_user_raw(discord_user_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingList {
    pub text_rules: Vec<TextMappingRule>,
    pub emoji_rules: Vec<EmojiMappingRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMappingRule {
    pub pattern: String,
    pub replacement: String,
    pub case_sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmojiMappingRule {
    pub emoji_id: String,
    pub emoji_name: String,
    pub emoji_image_url: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallerInfo {
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapperList {
    pub previous: Vec<MapperSummary>,
    pub future: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapperSummary {
    pub id: String,
    pub name: String,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub username: String,
    #[serde(default)]
    pub global_nickname: Option<String>,
    #[serde(default)]
    pub guild_nickname: Option<String>,
}

/// Output from a WASM mapper — the response to send back to the host
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    /// Transformed text (may be empty)
    pub text: String,
    /// Optional error message (if set, text is ignored but still sent)
    #[serde(default)]
    pub error: Option<String>,
    /// Optional override of the remaining pipeline (None = continue as normal)
    #[serde(default)]
    pub override_future_mappers: Option<Vec<String>>,
}

impl Output {
    /// Create a success output with transformed text
    pub fn text(t: impl Into<String>) -> Self {
        Self {
            text: t.into(),
            error: None,
            override_future_mappers: None,
        }
    }

    /// Create an error output
    pub fn error(e: impl Into<String>) -> Self {
        Self {
            text: String::new(),
            error: Some(e.into()),
            override_future_mappers: None,
        }
    }

    /// Set which mappers should execute next, overriding the pipeline
    pub fn override_pipeline(mut self, mapper_ids: Vec<String>) -> Self {
        self.override_future_mappers = Some(mapper_ids);
        self
    }
}
