use crate::middleware::auth::AuthUser;
use axum::{Json, extract::{Path, Query, State}};
use hq_core::{CoreError, Service};
use hq_types::hq::{AuthUserDto, PaginatedResponseDto, TapWithAccessDto};
use hq_types::hq::settings::{PartialUserSettings, UserSettings};
use serde::Deserialize;
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

#[utoipa::path(
    get,
    path = "/api/v1/users/me",
    responses(
        (status = 200, description = "Current user profile", body = AuthUserDto)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_me(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<AuthUserDto>, (axum::http::StatusCode, String)> {
    let user = service
        .auth
        .get_user(&user_id.to_string())
        .await
        .map_err(map_error)?;

    Ok(Json(user))
}

#[utoipa::path(
    get,
    path = "/api/v1/users/me/taps",
    responses(
        (status = 200, description = "List of current user's taps", body = PaginatedResponseDto<TapWithAccessDto>)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_my_taps(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<PaginatedResponseDto<TapWithAccessDto>>, (axum::http::StatusCode, String)> {
    let taps = service.tap.list_by_user(user_id).await.map_err(map_error)?;

    Ok(Json(taps))
}

// --- User-scope settings ---

#[utoipa::path(
    get,
    path = "/api/v1/users/me/settings",
    responses(
        (status = 200, description = "Current user settings (User scope)", body = PartialUserSettings)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_my_settings(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<PartialUserSettings>, (axum::http::StatusCode, String)> {
    let settings = service
        .user_settings
        .get_settings(user_id)
        .await
        .map_err(map_error)?;
    Ok(Json(settings))
}

#[utoipa::path(
    put,
    path = "/api/v1/users/me/settings",
    request_body = PartialUserSettings,
    responses(
        (status = 200, description = "Updated user settings (User scope)", body = PartialUserSettings)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_my_settings(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Json(body): Json<PartialUserSettings>,
) -> Result<Json<PartialUserSettings>, (axum::http::StatusCode, String)> {
    let settings = service
        .user_settings
        .save_settings(user_id, body)
        .await
        .map_err(map_error)?;
    Ok(Json(settings))
}

// --- GuildUser-scope settings ---

#[utoipa::path(
    get,
    path = "/api/v1/users/me/settings/guilds/{guild_id}",
    params(("guild_id" = String, Path, description = "Discord guild ID")),
    responses(
        (status = 200, description = "Guild-user settings override", body = PartialUserSettings),
        (status = 404, description = "No override set for this guild")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_my_guild_settings(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Path(guild_id): Path<String>,
) -> Result<Json<PartialUserSettings>, (axum::http::StatusCode, String)> {
    let settings = service
        .user_settings
        .get_guild_user_settings(&user_id, &guild_id)
        .await
        .map_err(map_error)?
        .ok_or_else(|| (axum::http::StatusCode::NOT_FOUND, "No settings override for this guild".into()))?;
    Ok(Json(settings))
}

#[utoipa::path(
    put,
    path = "/api/v1/users/me/settings/guilds/{guild_id}",
    params(("guild_id" = String, Path, description = "Discord guild ID")),
    request_body = PartialUserSettings,
    responses(
        (status = 200, description = "Updated guild-user settings override", body = PartialUserSettings)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_my_guild_settings(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Path(guild_id): Path<String>,
    Json(body): Json<PartialUserSettings>,
) -> Result<Json<PartialUserSettings>, (axum::http::StatusCode, String)> {
    let settings = service
        .user_settings
        .save_guild_user_settings(&user_id, &guild_id, body)
        .await
        .map_err(map_error)?;
    Ok(Json(settings))
}

#[utoipa::path(
    delete,
    path = "/api/v1/users/me/settings/guilds/{guild_id}",
    params(("guild_id" = String, Path, description = "Discord guild ID")),
    responses(
        (status = 204, description = "Guild-user settings override deleted")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn delete_my_guild_settings(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Path(guild_id): Path<String>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
    service
        .user_settings
        .delete_guild_user_settings(&user_id, &guild_id)
        .await
        .map_err(map_error)?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// --- Effective (resolved) settings ---

#[derive(Deserialize)]
pub struct EffectiveSettingsQuery {
    pub guild_id: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/users/me/settings/effective",
    params(("guild_id" = String, Query, description = "Discord guild ID to resolve settings for")),
    responses(
        (status = 200, description = "Fully resolved settings for the given guild context", body = UserSettings)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_effective_settings(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Query(query): Query<EffectiveSettingsQuery>,
) -> Result<Json<UserSettings>, (axum::http::StatusCode, String)> {
    let settings = service
        .user_settings
        .get_effective_settings(&user_id, &query.guild_id)
        .await
        .map_err(map_error)?;
    Ok(Json(settings))
}
