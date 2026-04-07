use crate::middleware::auth::AuthUser;
use axum::{Json, extract::{Path, State}};
use hq_core::{CoreError, Service};
use hq_types::hq::settings::PartialUserSettings;
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

// --- Guild scope (admin only) ---

#[utoipa::path(
    get,
    path = "/api/v1/guilds/{guild_id}/settings",
    params(("guild_id" = String, Path, description = "Discord guild ID")),
    responses(
        (status = 200, description = "Guild-wide settings", body = PartialUserSettings),
        (status = 404, description = "No guild settings configured")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_guild_settings(
    State(service): State<Arc<Service>>,
    AuthUser(_user_id): AuthUser,
    Path(guild_id): Path<String>,
) -> Result<Json<PartialUserSettings>, (axum::http::StatusCode, String)> {
    let settings = service
        .user_settings
        .get_guild_settings(&guild_id)
        .await
        .map_err(map_error)?
        .ok_or_else(|| (axum::http::StatusCode::NOT_FOUND, "No settings configured for this guild".into()))?;
    Ok(Json(settings))
}

#[utoipa::path(
    put,
    path = "/api/v1/guilds/{guild_id}/settings",
    params(("guild_id" = String, Path, description = "Discord guild ID")),
    request_body = PartialUserSettings,
    responses(
        (status = 200, description = "Updated guild-wide settings", body = PartialUserSettings)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_guild_settings(
    State(service): State<Arc<Service>>,
    AuthUser(_user_id): AuthUser,
    Path(guild_id): Path<String>,
    Json(body): Json<PartialUserSettings>,
) -> Result<Json<PartialUserSettings>, (axum::http::StatusCode, String)> {
    let settings = service
        .user_settings
        .save_guild_settings(&guild_id, body)
        .await
        .map_err(map_error)?;
    Ok(Json(settings))
}

// --- Global scope (admin only) ---

#[utoipa::path(
    get,
    path = "/api/v1/settings/global",
    responses(
        (status = 200, description = "Global settings baseline", body = PartialUserSettings),
        (status = 404, description = "No global settings configured")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_global_settings(
    State(service): State<Arc<Service>>,
    AuthUser(_user_id): AuthUser,
) -> Result<Json<PartialUserSettings>, (axum::http::StatusCode, String)> {
    let settings = service
        .user_settings
        .get_global_settings()
        .await
        .map_err(map_error)?
        .ok_or_else(|| (axum::http::StatusCode::NOT_FOUND, "No global settings configured".into()))?;
    Ok(Json(settings))
}

#[utoipa::path(
    put,
    path = "/api/v1/settings/global",
    request_body = PartialUserSettings,
    responses(
        (status = 200, description = "Updated global settings", body = PartialUserSettings)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_global_settings(
    State(service): State<Arc<Service>>,
    AuthUser(_user_id): AuthUser,
    Json(body): Json<PartialUserSettings>,
) -> Result<Json<PartialUserSettings>, (axum::http::StatusCode, String)> {
    let settings = service
        .user_settings
        .save_global_settings(body)
        .await
        .map_err(map_error)?;
    Ok(Json(settings))
}
