use crate::middleware::auth::AuthUser;
use axum::{extract::Query, extract::State, response::Redirect, Json};
use hq_core::Service;
use hq_types::hq::{AuthCallbackDto, AuthResponseDto};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use utoipa;

#[derive(Deserialize)]
pub struct LoginQuery {
    pub redirect: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/login",
    params(
        ("redirect" = Option<String>, Query, description = "Path to redirect to after login")
    ),
    responses(
        (status = 302, description = "Redirect to Discord OAuth2 authorize")
    )
)]
pub async fn login_handler(
    State(service): State<Arc<Service>>,
    Query(query): Query<LoginQuery>,
) -> Redirect {
    let url = service.auth.get_login_url(query.redirect.as_deref());
    Redirect::temporary(&url)
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/callback",
    params(
        ("code" = String, Query, description = "Discord OAuth2 code"),
        ("state" = Option<String>, Query, description = "OAuth2 state (encoded redirect path)")
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
        .authenticate(&payload.code, payload.state.as_deref())
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
