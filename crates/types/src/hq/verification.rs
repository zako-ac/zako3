use super::{Tap, TapId, UserId};
use derive_more::{Display, From, FromStr, Into};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    From,
    Into,
    PartialEq,
    Eq,
    ToSchema,
    Hash,
    FromStr,
    Display,
)]
pub struct VerificationRequestId(pub String);

#[derive(
    Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema, PartialEq, Eq,
)]
#[serde(rename_all = "lowercase")]
pub enum VerificationStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerificationRequest {
    pub id: VerificationRequestId,
    pub tap_id: TapId,
    pub tap: Option<Tap>,
    pub requester_id: UserId,
    pub title: String,
    pub description: String,
    pub status: VerificationStatus,
    pub rejection_reason: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateVerificationRequestDto {
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RejectVerificationDto {
    pub reason: String,
}
