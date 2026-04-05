use crate::middleware::auth::AuthUser;
use axum::{Json, extract::Query, extract::State};
use hq_core::Service;
use hq_types::hq::{AuthCallbackDto, AuthResponseDto, LoginResponseDto};
use serde_json::json;
use std::sync::Arc;
use utoipa;

#[utoipa::path(
    get,
    path = "/api/v1/auth/login",
    responses(
        (status = 200, description = "Get discord login url", body = LoginResponseDto)
    )
)]
pub async fn login_handler(
    State(service): State<Arc<Service>>,
) -> Result<Json<LoginResponseDto>, (axum::http::StatusCode, String)> {
    let url = service.auth.get_login_url();
    Ok(Json(url))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/callback",
    params(
        ("code" = String, Query, description = "Discord OAuth2 code")
    ),
    responses(
        (status = 200, description = "Login successful", body = AuthResponseDto)
    )
)]
pub async fn callback_handler(
    State(service): State<Arc<Service>>,
    Query(payload): Query<AuthCallbackDto>,
) -> Result<Json<AuthResponseDto>, (axum::http::StatusCode, String)> {
    let response = service
        .auth
        .authenticate(&payload.code)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/refresh",
    responses(
        (status = 200, description = "Refresh successful", body = AuthResponseDto)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn refresh_handler(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<AuthResponseDto>, (axum::http::StatusCode, String)> {
    let response = service
        .auth
        .refresh_token(&user_id.to_string())
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    responses(
        (status = 200, description = "Logout successful")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn logout_handler(
    AuthUser(_user_id): AuthUser,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    // For MVP, just return 200 OK since JWTs are stateless on backend
    Ok(Json(json!({ "success": true })))
}
