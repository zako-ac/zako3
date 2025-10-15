use std::time::SystemTimeError;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use sqlx::types::Json;
use thiserror::Error;
use utoipa::ToSchema;

use crate::core::auth::error::AuthError;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("unknown error: {0}")]
    Unknown(String),

    #[error("DB transaction error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Time went backwards: {0}")]
    Time(#[from] SystemTimeError),

    #[error("JWT serialization error: {0}")]
    Jwt(#[from] jwt::Error),

    #[error("Unauthorized: {0}")]
    Auth(#[from] AuthError),

    #[error("Resource not found")]
    NotFound,
}

pub type AppResult<T> = Result<T, AppError>;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        to_response_error(self).into_response()
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ResponseError {
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

fn unauthorized(message: &str) -> (StatusCode, Json<ResponseError>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(ResponseError {
            kind: "unauthorized".to_string(),
            message: message.to_string(),
        }),
    )
}

fn not_found() -> (StatusCode, Json<ResponseError>) {
    (
        StatusCode::NOT_FOUND,
        Json(ResponseError {
            kind: "not_found".to_string(),
            message: "Not found".to_string(),
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
            tracing::warn!(event = "error", kind = "serde", error = %error.to_string());
            internal_error("unknown", "internal server error")
        }
        AppError::Redis(error) => {
            tracing::warn!(event = "error", kind = "redis", error = %error.to_string());
            internal_error("unknown", "internal server error")
        }
        AppError::Time(error) => {
            tracing::warn!(event = "error", kind = "time", error = %error.to_string());
            internal_error("unknown", "internal server error")
        }
        AppError::Jwt(error) => {
            tracing::warn!(event = "error", kind = "jwt", error = %error.to_string());
            internal_error("unknown", "internal server error")
        }
        AppError::Auth(error) => {
            tracing::warn!(event = "auth", kind = "fail", error = %error.to_string());
            unauthorized(&error.to_string())
        }
        AppError::NotFound => not_found(),
    }
}
