use crate::middleware::auth::AuthUser;
use axum::{extract::State, Json};
use hq_core::{CoreError, Service};
use hq_types::hq::{CreateTapDto, Tap};
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
    post,
    path = "/api/v1/taps",
    request_body = CreateTapDto,
    responses(
        (status = 200, description = "Tap created", body = Tap)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_tap(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Json(payload): Json<CreateTapDto>,
) -> Result<Json<Tap>, (axum::http::StatusCode, String)> {
    let tap = service
        .tap
        .create(user_id, payload)
        .await
        .map_err(map_error)?;

    Ok(Json(tap))
}

#[utoipa::path(
    get,
    path = "/api/v1/taps",
    responses(
        (status = 200, description = "List of taps", body = Vec<Tap>)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_taps(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<Vec<Tap>>, (axum::http::StatusCode, String)> {
    let taps = service.tap.list_by_user(user_id).await.map_err(map_error)?;

    Ok(Json(taps))
}
