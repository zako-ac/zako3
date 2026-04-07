use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct GuildSummaryDto {
    pub guild_id: String,
    pub guild_name: String,
    pub guild_icon_url: Option<String>,
    pub active_channel_id: Option<String>,
    pub active_channel_name: Option<String>,
    pub can_manage: bool,
}
