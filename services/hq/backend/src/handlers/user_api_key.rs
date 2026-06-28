use crate::middleware::auth::AuthUser;
use axum::{
    Json,
    extract::{Path, State},
};
use hq_core::{CoreError, Service};
use hq_types::hq::{
    CreateUserApiKeyDto, UpdateUserApiKeyDto, UserApiKeyDto, UserApiKeyId, UserApiKeyResponseDto,
};
use std::sync::Arc;

fn map_error(e: CoreError) -> (axum::http::StatusCode, String) {
    match e {
        CoreError::NotFound(_) => (axum::http::StatusCode::NOT_FOUND, e.to_string()),
        CoreError::InvalidInput(_) => (axum::http::StatusCode::BAD_REQUEST, e.to_string()),
        CoreError::Unauthorized(_) => (axum::http::StatusCode::UNAUTHORIZED, e.to_string()),
        CoreError::Forbidden(_) => (axum::http::StatusCode::FORBIDDEN, e.to_string()),
        CoreError::Conflict(_) => (axum::http::StatusCode::CONFLICT, e.to_string()),
        _ => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

pub async fn create_key(
    State(service): State<Arc<Service>>,
    AuthUser(user): AuthUser,
    Json(dto): Json<CreateUserApiKeyDto>,
) -> Result<Json<UserApiKeyResponseDto>, (axum::http::StatusCode, String)> {
    let res = service
        .user_api_key
        .create_key(user, dto)
        .await
        .map_err(map_error)?;
    Ok(Json(res))
}

pub async fn list_keys(
    State(service): State<Arc<Service>>,
    AuthUser(user): AuthUser,
) -> Result<Json<Vec<UserApiKeyDto>>, (axum::http::StatusCode, String)> {
    let res = service
        .user_api_key
        .list_keys(user)
        .await
        .map_err(map_error)?;
    Ok(Json(res))
}

pub async fn update_key(
    State(service): State<Arc<Service>>,
    AuthUser(user): AuthUser,
    Path(key_id): Path<UserApiKeyId>,
    Json(dto): Json<UpdateUserApiKeyDto>,
) -> Result<Json<UserApiKeyDto>, (axum::http::StatusCode, String)> {
    let res = service
        .user_api_key
        .update_key(user, key_id, dto)
        .await
        .map_err(map_error)?;
    Ok(Json(res))
}

pub async fn revoke_key(
    State(service): State<Arc<Service>>,
    AuthUser(user): AuthUser,
    Path(key_id): Path<UserApiKeyId>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
    service
        .user_api_key
        .revoke_key(user, key_id)
        .await
        .map_err(map_error)?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
