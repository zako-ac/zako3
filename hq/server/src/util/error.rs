use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("unknown error: {0}")]
    Unknown(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        to_response_error(self).into_response()
    }
}

#[derive(Debug, Clone, Serialize)]
struct ResponseError {
    pub kind: String,
    pub message: String,
}

fn to_response_error(app_err: AppError) -> (StatusCode, Json<ResponseError>) {
    match app_err {
        AppError::Unknown(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ResponseError {
                kind: "Unknown".into(),
                message,
            }),
        ),
    }
}
