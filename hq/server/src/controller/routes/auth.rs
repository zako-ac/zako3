use axum::{Json, extract::State};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    controller::helper::{AppResponse, into_app_response},
    core::app::AppState,
    feature::{auth::types::JwtPair, token::service::TokenService},
    util::error::ResponseError,
};

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TokenRefreshRequest {
    pub refresh_token: String,
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

    let refresh_result = app.service.refresh_user_token(&refresh_token).await?;

    into_app_response(refresh_result)
}
