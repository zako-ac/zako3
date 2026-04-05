use crate::hq::dtos::PaginationMetaDto;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub type AuditLogId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditLog {
    pub id: AuditLogId,
    pub tap_id: Uuid,
    pub actor_id: Uuid,
    pub action_type: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogDto {
    pub id: String,
    pub tap_id: String,
    pub actor_id: String,
    pub action_type: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateAuditLogDto {
    pub tap_id: Uuid,
    pub actor_id: Uuid,
    pub action_type: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedAuditLogsDto {
    pub data: Vec<AuditLogDto>,
    pub meta: PaginationMetaDto,
}
