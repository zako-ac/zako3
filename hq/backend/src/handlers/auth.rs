use axum::{Json, extract::Query, extract::State};
use hq_core::Service;
use hq_types::hq::{AuthCallbackDto, AuthResponseDto, LoginResponseDto};
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
