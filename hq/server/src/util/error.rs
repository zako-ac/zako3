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

    #[error("DB transaction error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),
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

fn internal_error(kind: &str, message: &str) -> (StatusCode, Json<ResponseError>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ResponseError {
            kind: kind.to_string(),
            message: message.to_string(),
        }),
    )
}

fn to_response_error(app_err: AppError) -> (StatusCode, Json<ResponseError>) {
    match app_err {
        AppError::Unknown(message) => {
            tracing::warn!(event = "error", kind = "unknown", %message);

            internal_error("unknown", "internal server error")
        }
        AppError::Sqlx(error) => {
            tracing::warn!(event = "error", kind = "sqlx", error = %error.to_string());
            internal_error("unknown", "internal server error")
        }
        AppError::SerdeJson(error) => {
            tracing::warn!(event = "error", kind = "sqlx", error = %error.to_string());
            internal_error("unknown", "internal server error")
        }
    }
}
