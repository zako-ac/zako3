use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq, ToSchema)]
pub struct ResourceTimestamp {
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: DateTime<Utc>,

    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub updated_at: DateTime<Utc>,
}

impl ResourceTimestamp {
    pub fn now() -> Self {
        Self {
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}
