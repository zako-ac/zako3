use super::UserId;
use derive_more::{From, FromStr, Into};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Debug, Clone, Serialize, Deserialize, From, Into, PartialEq, Eq, ToSchema, Hash, FromStr, Copy,
)]
pub struct NotificationId(pub u64);

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Notification {
    pub id: NotificationId,
    pub user_id: UserId,
    pub r#type: String,
    pub title: String,
    pub message: String,
    pub read_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Notification {
    pub fn new(id: u64, user_id: u64, r#type: String, title: String, message: String) -> Self {
        Self {
            id: NotificationId(id),
            user_id: UserId(user_id),
            r#type,
            title,
            message,
            read_at: None,
            created_at: chrono::Utc::now(),
        }
    }
}
