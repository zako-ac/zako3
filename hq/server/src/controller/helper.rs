use axum::{
    Json,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct OkResponse {
    #[schema(example = true)]
    ok: bool,
}

impl Default for OkResponse {
    fn default() -> Self {
        Self::new()
    }
}

impl OkResponse {
    pub fn new() -> Self {
        Self { ok: true }
    }
}

impl IntoResponse for OkResponse {
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}
