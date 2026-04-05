use crate::middleware::auth::AdminUser;
use axum::{
    Json,
    extract::{Path, State},
};
use hq_core::{CoreError, Service};
use hq_types::hq::{TapDto, TapOccupation};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct VerifyTapDto {
    pub occupation: TapOccupation,
}

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
    path = "/api/v1/admin/taps/{id}/verify",
    request_body = VerifyTapDto,
    params(
        ("id" = Uuid, Path, description = "Tap ID"),
    ),
    responses(
        (status = 200, description = "Tap verified", body = TapDto)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn verify_tap(
    State(service): State<Arc<Service>>,
    AdminUser(admin_id): AdminUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<VerifyTapDto>,
) -> Result<Json<TapDto>, (axum::http::StatusCode, String)> {
    let tap = service
        .tap
        .verify_tap(id, admin_id, payload.occupation)
        .await
        .map_err(map_error)?;

    Ok(Json(TapDto {
        id: tap.id.0.to_string(),
        name: tap.name.0.clone(),
        description: tap.description.clone().unwrap_or_default(),
        owner_id: tap.owner_id.0.to_string(),
        occupation: tap.occupation.clone(),
        permission: tap.permission.clone(),
        roles: tap.roles.clone(),
        total_uses: 0,
        created_at: tap.timestamp.created_at,
        updated_at: tap.timestamp.updated_at,
    }))
}
