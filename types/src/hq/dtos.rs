use super::tap::{TapOccupation, TapPermission, TapRole};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateTapDto {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AuthCallbackDto {
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthUserDto {
    pub id: String,
    pub discord_id: String,
    pub username: String,
    pub avatar: String,
    pub email: Option<String>,
    pub is_admin: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AuthResponseDto {
    pub token: String,
    pub user: AuthUserDto,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponseDto {
    pub redirect_url: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserSummaryDto {
    pub id: String,
    pub username: String,
    pub avatar: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TapDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner_id: String,
    pub occupation: TapOccupation,
    pub permission: TapPermission,
    pub roles: Vec<TapRole>,
    pub total_uses: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TapWithAccessDto {
    #[serde(flatten)]
    pub tap: TapDto,
    pub has_access: bool,
    pub owner: UserSummaryDto,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimeSeriesPointDto {
    pub timestamp: String,
    pub value: f64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TapStatsDto {
    pub tap_id: String,
    pub currently_active: u64,
    pub total_uses: u64,
    pub cache_hits: u64,
    pub unique_users: u64,
    pub use_rate_history: Vec<TimeSeriesPointDto>,
    pub cache_hit_rate_history: Vec<TimeSeriesPointDto>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PaginationMetaDto {
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub total_pages: u64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponseDto<T> {
    pub data: Vec<T>,
    pub meta: PaginationMetaDto,
}
