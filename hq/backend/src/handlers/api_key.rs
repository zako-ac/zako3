use crate::middleware::auth::AuthUser;
use axum::{
    Json,
    extract::{Path, State},
};
use hq_core::{CoreError, Service};
use hq_types::hq::{ApiKeyDto, ApiKeyResponseDto, CreateApiKeyDto, UpdateApiKeyDto};
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
    Path(tap_id): Path<u64>,
    Json(dto): Json<CreateApiKeyDto>,
) -> Result<Json<ApiKeyResponseDto>, (axum::http::StatusCode, String)> {
    let res = service
        .api_key
        .create_key(tap_id, user, dto)
        .await
        .map_err(map_error)?;
    Ok(Json(res))
}

pub async fn list_keys(
    State(service): State<Arc<Service>>,
    AuthUser(user): AuthUser,
    Path(tap_id): Path<u64>,
) -> Result<Json<Vec<ApiKeyDto>>, (axum::http::StatusCode, String)> {
    let res = service
        .api_key
        .list_keys(tap_id, user)
        .await
        .map_err(map_error)?;
    Ok(Json(res))
}

pub async fn update_key(
    State(service): State<Arc<Service>>,
    AuthUser(user): AuthUser,
    Path((tap_id, key_id)): Path<(u64, u64)>,
    Json(dto): Json<UpdateApiKeyDto>,
) -> Result<Json<ApiKeyDto>, (axum::http::StatusCode, String)> {
    let res = service
        .api_key
        .update_key(tap_id, key_id, user, dto)
        .await
        .map_err(map_error)?;
    Ok(Json(res))
}

pub async fn delete_key(
    State(service): State<Arc<Service>>,
    AuthUser(user): AuthUser,
    Path((tap_id, key_id)): Path<(u64, u64)>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
    service
        .api_key
        .delete_key(tap_id, key_id, user)
        .await
        .map_err(map_error)?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn regenerate_key(
    State(service): State<Arc<Service>>,
    AuthUser(user): AuthUser,
    Path((tap_id, key_id)): Path<(u64, u64)>,
) -> Result<Json<ApiKeyResponseDto>, (axum::http::StatusCode, String)> {
    let res = service
        .api_key
        .regenerate_key(tap_id, key_id, user)
        .await
        .map_err(map_error)?;
    Ok(Json(res))
}
