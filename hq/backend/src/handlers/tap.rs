use crate::middleware::auth::AuthUser;
use axum::{
    Json,
    extract::{Path, State},
};
use hq_core::{CoreError, Service};
use hq_types::hq::{CreateTapDto, PaginatedResponseDto, Tap, TapStatsDto, TapWithAccessDto};
use std::sync::Arc;
use uuid::Uuid;

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
        (status = 200, description = "List of taps", body = PaginatedResponseDto<TapWithAccessDto>)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_taps(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<PaginatedResponseDto<TapWithAccessDto>>, (axum::http::StatusCode, String)> {
    let taps = service.tap.list_by_user(user_id).await.map_err(map_error)?;

    Ok(Json(taps))
}

#[utoipa::path(
    get,
    path = "/api/v1/taps/{id}",
    params(
        ("id" = Uuid, Path, description = "Tap ID")
    ),
    responses(
        (status = 200, description = "Tap details", body = TapWithAccessDto)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_tap(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Path(tap_id): Path<Uuid>,
) -> Result<Json<TapWithAccessDto>, (axum::http::StatusCode, String)> {
    let tap = service
        .tap
        .get_tap_with_access(tap_id, user_id)
        .await
        .map_err(map_error)?;

    Ok(Json(tap))
}

#[utoipa::path(
    get,
    path = "/api/v1/taps/{id}/stats",
    params(
        ("id" = Uuid, Path, description = "Tap ID")
    ),
    responses(
        (status = 200, description = "Tap statistics", body = TapStatsDto)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_tap_stats(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Path(tap_id): Path<Uuid>,
) -> Result<Json<TapStatsDto>, (axum::http::StatusCode, String)> {
    let stats = service
        .tap
        .get_tap_stats(tap_id, user_id)
        .await
        .map_err(map_error)?;

    Ok(Json(stats))
}

#[utoipa::path(
    patch,
    path = "/api/v1/taps/{id}",
    params(
        ("id" = Uuid, Path, description = "Tap ID")
    ),
    request_body = hq_types::hq::UpdateTapDto,
    responses(
        (status = 200, description = "Tap updated", body = Tap)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_tap(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Path(tap_id): Path<Uuid>,
    Json(payload): Json<hq_types::hq::UpdateTapDto>,
) -> Result<Json<Tap>, (axum::http::StatusCode, String)> {
    let tap = service
        .tap
        .update_tap(tap_id, user_id, payload)
        .await
        .map_err(map_error)?;

    Ok(Json(tap))
}

#[utoipa::path(
    delete,
    path = "/api/v1/taps/{id}",
    params(
        ("id" = Uuid, Path, description = "Tap ID")
    ),
    responses(
        (status = 200, description = "Tap deleted")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn delete_tap(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Path(tap_id): Path<Uuid>,
) -> Result<Json<()>, (axum::http::StatusCode, String)> {
    service
        .tap
        .delete_tap(tap_id, user_id)
        .await
        .map_err(map_error)?;

    Ok(Json(()))
}
