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
pub struct AuthResponseDto {
    pub token: String,
}
