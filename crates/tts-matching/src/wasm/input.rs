use serde::{Deserialize, Serialize};
use zako3_types::hq::settings::{EmojiMappingRule, TextMappingRule};

use crate::model::MapperSummary;

#[derive(Debug, Serialize)]
pub(crate) struct WasmInput<'a> {
    pub text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapping_list: Option<MappingList<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caller_info: Option<CallerInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapper_list: Option<MapperListPayload<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<u64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct MappingList<'a> {
    pub text_rules: &'a [TextMappingRule],
    pub emoji_rules: &'a [EmojiMappingRule],
}

#[derive(Debug, Serialize)]
pub(crate) struct CallerInfo {
    pub user_id: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct MapperListPayload<'a> {
    pub previous: &'a [MapperSummary],
    pub future: &'a [String],
}

#[derive(Debug, Deserialize)]
pub(crate) struct WasmOutput {
    pub text: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub override_future_mappers: Option<Vec<String>>,
}
