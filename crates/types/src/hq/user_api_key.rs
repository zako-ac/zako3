use super::user::UserId;
use chrono::{DateTime, Utc};
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
pub struct UserApiKeyId(pub String);

/// A user-scoped personal access token. The id doubles as the JWT `jti`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserApiKey {
    pub id: UserApiKeyId,
    pub user_id: UserId,
    pub label: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
