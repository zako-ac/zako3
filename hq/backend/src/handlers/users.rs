use crate::middleware::auth::AuthUser;
use axum::{Json, extract::State};
use hq_core::{CoreError, Service};
use hq_types::hq::{AuthUserDto, PaginatedResponseDto, TapWithAccessDto};
use hq_types::hq::settings::UserSettings;
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

#[utoipa::path(
    get,
    path = "/api/v1/users/me/settings",
    responses(
        (status = 200, description = "Current user settings", body = UserSettings)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_my_settings(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<UserSettings>, (axum::http::StatusCode, String)> {
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
    request_body = UserSettings,
    responses(
        (status = 200, description = "Updated user settings", body = UserSettings)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_my_settings(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Json(body): Json<UserSettings>,
) -> Result<Json<UserSettings>, (axum::http::StatusCode, String)> {
    let settings = service
        .user_settings
        .save_settings(user_id, body)
        .await
        .map_err(map_error)?;
    Ok(Json(settings))
}
