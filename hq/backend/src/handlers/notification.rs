use crate::middleware::auth::AuthUser;
use axum::{
    Json,
    extract::{Path, State},
};
use hq_core::{CoreError, Service};
use hq_types::hq::{NotificationDto, NotificationId, PaginatedResponseDto};
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
    path = "/api/v1/notifications",
    responses(
        (status = 200, description = "List notifications")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_notifications(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<PaginatedResponseDto<NotificationDto>>, (axum::http::StatusCode, String)> {
    let notifications = service
        .notification
        .list_by_user(user_id)
        .await
        .map_err(map_error)?;
    Ok(Json(notifications))
}

#[utoipa::path(
    patch,
    path = "/api/v1/notifications/{id}/read",
    params(
        ("id" = String, Path, description = "Notification ID"),
    ),
    responses(
        (status = 200, description = "Notification marked as read", body = NotificationDto)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn mark_notification_read(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<NotificationId>,
) -> Result<Json<NotificationDto>, (axum::http::StatusCode, String)> {
    let notification = service
        .notification
        .mark_as_read(id, user_id)
        .await
        .map_err(map_error)?;
    Ok(Json(notification))
}
