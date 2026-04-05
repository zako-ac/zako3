use crate::middleware::auth::{AuthUser, OptionalAuthUser};
use axum::{
    Json,
    extract::{Path, State},
};
use hq_core::{CoreError, Service};
use hq_types::hq::{
    CreateTapDto, CreateVerificationRequestDto, PaginatedResponseDto, Tap, TapDto, TapId,
    TapStatsDto, TapWithAccessDto, VerificationRequest,
};
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
        (status = 200, description = "Tap created", body = TapDto)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_tap(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Json(payload): Json<CreateTapDto>,
) -> Result<Json<TapDto>, (axum::http::StatusCode, String)> {
    let tap = service
        .tap
        .create(user_id, payload)
        .await
        .map_err(map_error)?;

    let tap_dto = TapDto {
        id: tap.id.0.to_string(),
        name: tap.name.0,
        description: tap.description.unwrap_or_default(),
        owner_id: tap.owner_id.0.to_string(),
        occupation: tap.occupation,
        permission: tap.permission,
        roles: tap.roles,
        total_uses: 0,
        cache_hits: 0,
        created_at: tap.timestamp.created_at,
        updated_at: tap.timestamp.updated_at,
    };

    Ok(Json(tap_dto))
}

#[utoipa::path(
    get,
    path = "/api/v1/taps",
    responses(
        (status = 200, description = "List of taps", body = PaginatedResponseDto<TapWithAccessDto>)
    )
)]
pub async fn list_taps(
    State(service): State<Arc<Service>>,
    OptionalAuthUser(user_id): OptionalAuthUser,
) -> Result<Json<PaginatedResponseDto<TapWithAccessDto>>, (axum::http::StatusCode, String)> {
    let taps = service
        .tap
        .list_all_paginated(user_id)
        .await
        .map_err(map_error)?;

    Ok(Json(taps))
}

#[utoipa::path(
    get,
    path = "/api/v1/taps/{id}",
    params(
        ("id" = String, Path, description = "Tap ID")
    ),
    responses(
        (status = 200, description = "Tap details", body = TapWithAccessDto)
    )
)]
pub async fn get_tap(
    State(service): State<Arc<Service>>,
    OptionalAuthUser(user_id): OptionalAuthUser,
    Path(tap_id): Path<TapId>,
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
        ("id" = String, Path, description = "Tap ID")
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
    Path(tap_id): Path<TapId>,
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
        ("id" = String, Path, description = "Tap ID")
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
    Path(tap_id): Path<TapId>,
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
        ("id" = String, Path, description = "Tap ID")
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
    Path(tap_id): Path<TapId>,
) -> Result<Json<()>, (axum::http::StatusCode, String)> {
    service
        .tap
        .delete_tap(tap_id, user_id)
        .await
        .map_err(map_error)?;

    Ok(Json(()))
}

#[utoipa::path(
    post,
    path = "/api/v1/taps/{id}/verify",
    request_body = CreateVerificationRequestDto,
    params(
        ("id" = String, Path, description = "Tap ID")
    ),
    responses(
        (status = 200, description = "Verification requested", body = VerificationRequest)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn request_verification(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Path(tap_id): Path<TapId>,
    Json(payload): Json<CreateVerificationRequestDto>,
) -> Result<Json<VerificationRequest>, (axum::http::StatusCode, String)> {
    let request = service
        .verification
        .request_verification(tap_id, user_id, payload)
        .await
        .map_err(map_error)?;

    Ok(Json(request))
}
