use crate::middleware::auth::AdminUser;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use hq_core::{CoreError, Service};
use hq_types::hq::{
    RejectVerificationDto, VerificationRequest, VerificationRequestId, VerificationStatus,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct VerificationRequestsQuery {
    pub status: Option<VerificationStatus>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct PaginatedVerificationRequestsDto {
    pub data: Vec<VerificationRequest>,
    pub meta: hq_types::hq::dtos::PaginationMetaDto,
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
    get,
    path = "/api/v1/admin/verifications",
    params(
        ("status" = Option<VerificationStatus>, Query, description = "Filter by status"),
        ("page" = Option<u32>, Query, description = "Page number"),
        ("per_page" = Option<u32>, Query, description = "Items per page"),
    ),
    responses(
        (status = 200, description = "List of verification requests", body = PaginatedVerificationRequestsDto)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_verification_requests(
    State(service): State<Arc<Service>>,
    AdminUser(_admin_id): AdminUser,
    Query(query): Query<VerificationRequestsQuery>,
) -> Result<Json<PaginatedVerificationRequestsDto>, (axum::http::StatusCode, String)> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);

    let (requests, total) = service
        .verification
        .list_requests(query.status, page, per_page)
        .await
        .map_err(map_error)?;

    Ok(Json(PaginatedVerificationRequestsDto {
        data: requests,
        meta: hq_types::hq::dtos::PaginationMetaDto {
            total,
            page: page.into(),
            per_page: per_page.into(),
            total_pages: (total as f64 / per_page as f64).ceil() as u64,
        },
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/verifications/{id}/approve",
    params(
        ("id" = String, Path, description = "Request ID"),
    ),
    responses(
        (status = 200, description = "Verification approved", body = VerificationRequest)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn approve_verification(
    State(service): State<Arc<Service>>,
    AdminUser(admin_id): AdminUser,
    Path(id): Path<VerificationRequestId>,
) -> Result<Json<VerificationRequest>, (axum::http::StatusCode, String)> {
    let request = service
        .verification
        .approve_verification(id, admin_id)
        .await
        .map_err(map_error)?;

    Ok(Json(request))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/verifications/{id}/reject",
    request_body = RejectVerificationDto,
    params(
        ("id" = String, Path, description = "Request ID"),
    ),
    responses(
        (status = 200, description = "Verification rejected", body = VerificationRequest)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn reject_verification(
    State(service): State<Arc<Service>>,
    AdminUser(admin_id): AdminUser,
    Path(id): Path<VerificationRequestId>,
    Json(payload): Json<RejectVerificationDto>,
) -> Result<Json<VerificationRequest>, (axum::http::StatusCode, String)> {
    let request = service
        .verification
        .reject_verification(id, admin_id, payload.reason)
        .await
        .map_err(map_error)?;

    Ok(Json(request))
}
