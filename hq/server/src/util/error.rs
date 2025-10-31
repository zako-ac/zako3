use std::time::SystemTimeError;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;

use crate::feature::{auth::domain::error::AuthError, tap::error::TapError, user::error::UserError};

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

    #[error("Password hash error: {0}")]
    PasswordHash(#[from] password_hash::Error),

    #[error("Resource not found")]
    NotFound,

    #[error("business error: {0}")]
    Business(#[from] BusinessError),
}

#[derive(Error, Debug, Clone)]
pub enum BusinessError {
    #[error("user error: {0}")]
    User(#[from] UserError),
    #[error("tap error: {0}")]
    Tap(#[from] TapError),
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

fn invalid_request(message: &str) -> (StatusCode, Json<ResponseError>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ResponseError {
            kind: "bad_request".to_string(),
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
        // things that users should not see
        AppError::Unknown(message) => unknown_response_error("unknown", message),
        AppError::Sqlx(error) => unknown_response_error("sqlx", error),
        AppError::SerdeJson(error) => unknown_response_error("serde_json", error),
        AppError::Redis(error) => unknown_response_error("redis", error),
        AppError::Time(error) => unknown_response_error("time", error),
        AppError::Jwt(error) => unknown_response_error("jwt", error),
        AppError::PasswordHash(error) => unknown_response_error("password_hash", error),

        // things that users should see
        AppError::Auth(error) => {
            tracing::warn!(event = "auth", kind = "fail", error = %error.to_string());
            unauthorized(&error.to_string())
        }
        AppError::Business(error) => invalid_request(&error.to_string()),
        AppError::NotFound => not_found(),
    }
}

fn unknown_response_error(kind: &str, error: impl ToString) -> (StatusCode, Json<ResponseError>) {
    tracing::warn!(event = "error", kind = %kind, error = %error.to_string());
    internal_error("unknown", "internal server error")
}
