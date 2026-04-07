use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HubRejectReasonType {
    #[serde(rename = "unauthorized")]
    Unauthorized,

    #[serde(rename = "internal_error")]
    InternalError,
}
