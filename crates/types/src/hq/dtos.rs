use super::tap::{TapOccupation, TapPermission, TapRole};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTapDto {
    pub name: String,
    pub description: Option<String>,
    pub permission: Option<TapPermission>,
    pub roles: Option<Vec<TapRole>>,
    pub base_volume: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTapDto {
    pub name: Option<String>,
    pub description: Option<String>,
    pub permission: Option<TapPermission>,
    pub roles: Option<Vec<TapRole>>,
    pub base_volume: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOccupationDto {
    pub occupation: TapOccupation,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthCallbackDto {
    pub code: String,
    pub state: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthUserDto {
    pub id: String,
    pub discord_id: String,
    pub username: String,
    pub avatar: String,
    pub email: Option<String>,
    pub is_admin: bool,
    pub banned: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserRoleDto {
    pub role: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponseDto {
    pub token: String,
    pub user: AuthUserDto,
    pub redirect_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponseDto {
    pub redirect_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserSummaryDto {
    pub id: String,
    pub username: String,
    pub avatar: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct TapDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner_id: String,
    pub occupation: TapOccupation,
    pub permission: TapPermission,
    pub roles: Vec<TapRole>,
    pub base_volume: f32,
    pub total_uses: u64,
    pub cache_hits: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub stats: TapStatsDto,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct TapWithAccessDto {
    #[serde(flatten)]
    pub tap: TapDto,
    pub has_access: bool,
    pub owner: UserSummaryDto,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimeSeriesPointDto {
    pub timestamp: String,
    pub value: f64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct TapStatsDto {
    pub tap_id: String,
    pub currently_active: u64,
    pub total_uses: u64,
    pub cache_hits: u64,
    pub unique_users: u64,
    pub uptime_percent: f64,
    pub use_rate_history: Vec<TimeSeriesPointDto>,
    pub cache_hit_rate_history: Vec<TimeSeriesPointDto>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlatformStatsDto {
    pub global_unique_users: u64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
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

impl<T: zod_gen::ZodSchema> zod_gen::ZodSchema for PaginatedResponseDto<T> {
    fn zod_schema() -> String {
        zod_gen::zod_object(&[
            ("data", &zod_gen::zod_array(&T::zod_schema())),
            ("meta", &PaginationMetaDto::zod_schema()),
        ])
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiKeyDto {
    pub label: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApiKeyDto {
    pub label: Option<String>,
    pub expires_at: Option<Option<DateTime<Utc>>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyDto {
    pub id: String,
    pub tap_id: String,
    pub label: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyResponseDto {
    #[serde(flatten)]
    pub api_key: ApiKeyDto,
    pub token: String,
}

#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, zod_gen_derive::ZodSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct NotificationDto {
    pub id: String,
    pub user_id: String,
    pub r#type: String,
    pub title: String,
    pub message: String,
    pub read_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, zod_gen_derive::ZodSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct CreateNotificationDto {
    pub user_id: String,
    pub r#type: String,
    pub title: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(rename_all = "camelCase")]
pub struct UnreadCountDto {
    pub count: u64,
}
