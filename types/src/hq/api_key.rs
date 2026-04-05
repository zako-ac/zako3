use super::tap::TapId;
use chrono::{DateTime, Utc};
use derive_more::{From, FromStr, Into};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(
    Debug, Clone, Serialize, Deserialize, From, Into, PartialEq, Eq, ToSchema, Hash, FromStr,
)]
pub struct ApiKeyId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiKey {
    pub id: ApiKeyId,
    pub tap_id: TapId,
    pub name: String,
    pub key_hash: String,
    pub scopes: Vec<String>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
