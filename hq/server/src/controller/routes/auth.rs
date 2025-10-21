use axum::{Json, extract::State};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    controller::helper::{AppResponse, into_app_response},
    core::app::AppState,
    feature::auth::domain::model::JwtPair,
    util::error::ResponseError,
};

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TokenRefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TestLoginRequest {
    pub password: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    summary = "Refresh an access token",
    request_body = TokenRefreshRequest,
    tag = "auth",
    responses(
        ( status = 200, description = "JWT pair", body = JwtPair ),
        ( status = 401, description = "Unauthorized", body = ResponseError ),
    ),
    security(
    )
)]
pub async fn refresh_refresh_token(
    State(app): State<AppState>,
    Json(refresh_req): Json<TokenRefreshRequest>,
) -> AppResponse<JwtPair> {
    let refresh_token = refresh_req.refresh_token;

    let refresh_result = app
        .service
        .auth_service
        .refresh_user_token(&refresh_token)
        .await?;

    into_app_response(refresh_result)
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/test_login",
    summary = "Login",
    request_body = TestLoginRequest,
    tag = "auth",
    responses(
        ( status = 200, description = "JWT pair", body = JwtPair ),
        ( status = 401, description = "Unauthorized", body = ResponseError ),
    ),
    security(
    )
)]
pub async fn test_login(
    State(app): State<AppState>,
    Json(req): Json<TestLoginRequest>,
) -> AppResponse<JwtPair> {
    let r = app.service.auth_service.test_login(&req.password).await?;

    into_app_response(r)
}
