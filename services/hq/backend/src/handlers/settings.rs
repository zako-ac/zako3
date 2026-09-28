use crate::middleware::auth::{AdminUser, AuthUser};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use hq_core::{CoreError, Service};
use hq_types::hq::UserId;
use hq_types::hq::settings::PartialUserSettings;
use std::sync::Arc;

fn map_error(e: CoreError) -> (StatusCode, String) {
    match e {
        CoreError::NotFound(_) => (StatusCode::NOT_FOUND, e.to_string()),
        CoreError::InvalidInput(_) => (StatusCode::BAD_REQUEST, e.to_string()),
        CoreError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, e.to_string()),
        CoreError::Forbidden(_) => (StatusCode::FORBIDDEN, e.to_string()),
        CoreError::Conflict(_) => (StatusCode::CONFLICT, e.to_string()),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

// --- Guild scope (guild administrators) ---

/// Guild settings belong to the guild's Discord administrators
/// (`docs/settings.md`). The web client hides the editor from everyone else
/// (`canManage` from `GET /api/v1/guilds/me`), but that is a hint to the user,
/// not a permission check — so the write resolves it again, here.
async fn require_guild_admin(
    service: &Service,
    user_id: &UserId,
    guild_id: &str,
) -> Result<(), (StatusCode, String)> {
    let can_manage = service
        .user_can_manage_guild(user_id, guild_id)
        .await
        .map_err(map_error)?;

    if can_manage {
        Ok(())
    } else {
        Err((
            StatusCode::FORBIDDEN,
            "Guild administrator permissions required".to_string(),
        ))
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/guilds/{guild_id}/settings",
    params(("guild_id" = String, Path, description = "Discord guild ID")),
    responses(
        (status = 200, description = "Guild-wide settings", body = PartialUserSettings),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_guild_settings(
    State(service): State<Arc<Service>>,
    AuthUser(_user_id): AuthUser,
    Path(guild_id): Path<String>,
) -> Result<Json<PartialUserSettings>, (StatusCode, String)> {
    let settings = service
        .user_settings
        .get_guild_settings(&guild_id)
        .await
        .map_err(map_error)?
        .unwrap_or_default();

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
    AuthUser(user_id): AuthUser,
    Path(guild_id): Path<String>,
    Json(body): Json<PartialUserSettings>,
) -> Result<Json<PartialUserSettings>, (StatusCode, String)> {
    require_guild_admin(&service, &user_id, &guild_id).await?;

    let settings = service
        .user_settings
        .save_guild_settings(&guild_id, body)
        .await
        .map_err(map_error)?;
    Ok(Json(settings))
}

// --- Global scope (HQ admins) ---
//
// *Reading* the baseline stays open to every authenticated user, on purpose: the
// guild settings page folds it in for everyone as the upstream default. Writing
// it is an admin action, and `AdminUser` is what enforces that.

#[utoipa::path(
    get,
    path = "/api/v1/settings/global",
    responses(
        (status = 200, description = "Global settings baseline", body = PartialUserSettings),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_global_settings(
    State(service): State<Arc<Service>>,
    AuthUser(_user_id): AuthUser,
) -> Result<Json<PartialUserSettings>, (StatusCode, String)> {
    let settings = service
        .user_settings
        .get_global_settings()
        .await
        .map_err(map_error)?
        .unwrap_or_default();

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
    AdminUser(_admin_id): AdminUser,
    Json(body): Json<PartialUserSettings>,
) -> Result<Json<PartialUserSettings>, (StatusCode, String)> {
    let settings = service
        .user_settings
        .save_global_settings(body)
        .await
        .map_err(map_error)?;
    Ok(Json(settings))
}
