use axum::{
    Json,
    extract::{Path, Query, State},
};
use hq_core::Service;
use hq_types::hq::{TapId, audit_log::PaginatedAuditLogsDto};
use serde::Deserialize;
use std::sync::Arc;

use crate::middleware::auth::AuthUser;

#[derive(Deserialize)]
pub struct AuditLogQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/taps/{id}/audit-log",
    responses(
        (status = 200, description = "Audit logs retrieved successfully", body = PaginatedAuditLogsDto),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Tap not found")
    ),
    params(
        ("id" = String, Path, description = "Tap ID"),
        ("page" = Option<i64>, Query, description = "Page number (default 1)"),
        ("limit" = Option<i64>, Query, description = "Items per page (default 50)")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_tap_audit_logs(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Path(id): Path<TapId>,
    Query(query): Query<AuditLogQuery>,
) -> Result<Json<PaginatedAuditLogsDto>, axum::http::StatusCode> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(50).clamp(1, 100);

    let tap = service
        .tap
        .get_tap_with_access(id.clone(), Some(user_id))
        .await
        .map_err(|e| match e {
            hq_core::CoreError::NotFound(_) => axum::http::StatusCode::NOT_FOUND,
            hq_core::CoreError::Forbidden(_) => axum::http::StatusCode::FORBIDDEN,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    if !tap.has_access {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }

    let logs = service
        .audit_log
        .get_tap_logs(id, page, limit)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(logs))
}
