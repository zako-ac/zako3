use axum::{
    Json,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

use crate::util::error::AppResult;

pub type AppResponse<T> = AppResult<Json<T>>;
pub type AppOkResponse = AppResponse<OkResponse>;

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

pub fn ok_app_response() -> AppOkResponse {
    Ok(Json(OkResponse::new()))
}

pub fn into_app_response<T>(value: T) -> AppResponse<T> {
    Ok(Json(value))
}
