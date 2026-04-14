use crate::middleware::auth::AdminUser;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use hq_core::{CoreError, Service};
use hq_types::hq::{
    AuthUserDto, PaginatedResponseDto, PlatformStatsDto, RejectVerificationDto, UpdateUserRoleDto, UserId,
    VerificationRequest, VerificationRequestId, VerificationStatus,
};
use hq_types::hq::settings::PartialUserSettings;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct AdminUsersQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct VerificationRequestsQuery {
    pub status: Option<VerificationStatus>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Serialize, utoipa::ToSchema)]
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
    path = "/api/v1/admin/users",
    params(
        ("page" = Option<u32>, Query, description = "Page number"),
        ("per_page" = Option<u32>, Query, description = "Items per page"),
    ),
    responses(
        (status = 200, description = "List of users", body = PaginatedResponseDto<AuthUserDto>)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_users(
    State(service): State<Arc<Service>>,
    AdminUser(_admin_id): AdminUser,
    Query(query): Query<AdminUsersQuery>,
) -> Result<Json<PaginatedResponseDto<AuthUserDto>>, (axum::http::StatusCode, String)> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);

    let (users, total) = service
        .auth
        .list_all_users(page, per_page)
        .await
        .map_err(map_error)?;

    let user_dtos = users
        .into_iter()
        .map(|user| AuthUserDto {
            id: user.id.0.clone(),
            discord_id: user.discord_user_id.0.clone(),
            username: user.username.0.clone(),
            avatar: user.avatar_url.unwrap_or_default(),
            email: user.email.clone(),
            is_admin: user.permissions.contains(&"admin".to_string()),
            banned: user.banned,
        })
        .collect();

    Ok(Json(PaginatedResponseDto {
        data: user_dtos,
        meta: hq_types::hq::dtos::PaginationMetaDto {
            total,
            page: page.into(),
            per_page: per_page.into(),
            total_pages: (total as f64 / per_page as f64).ceil() as u64,
        },
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/users/{id}",
    params(
        ("id" = String, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "User details", body = AuthUserDto)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_user(
    State(service): State<Arc<Service>>,
    AdminUser(_admin_id): AdminUser,
    Path(id): Path<String>,
) -> Result<Json<AuthUserDto>, (axum::http::StatusCode, String)> {
    let user = service.auth.get_user(&id).await.map_err(map_error)?;

    Ok(Json(user))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/users/{id}/ban",
    params(
        ("id" = String, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "User banned", body = AuthUserDto)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn ban_user(
    State(service): State<Arc<Service>>,
    AdminUser(_admin_id): AdminUser,
    Path(id): Path<String>,
) -> Result<Json<AuthUserDto>, (axum::http::StatusCode, String)> {
    let user_id = UserId::from_str(&id).map_err(|_| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            "Invalid user ID".to_string(),
        )
    })?;

    let user = service.auth.ban_user(user_id).await.map_err(map_error)?;

    Ok(Json(AuthUserDto {
        id: user.id.0.clone(),
        discord_id: user.discord_user_id.0.clone(),
        username: user.username.0.clone(),
        avatar: user.avatar_url.unwrap_or_default(),
        email: user.email.clone(),
        is_admin: user.permissions.contains(&"admin".to_string()),
        banned: user.banned,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/users/{id}/unban",
    params(
        ("id" = String, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "User unbanned", body = AuthUserDto)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn unban_user(
    State(service): State<Arc<Service>>,
    AdminUser(_admin_id): AdminUser,
    Path(id): Path<String>,
) -> Result<Json<AuthUserDto>, (axum::http::StatusCode, String)> {
    let user_id = UserId::from_str(&id).map_err(|_| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            "Invalid user ID".to_string(),
        )
    })?;

    let user = service.auth.unban_user(user_id).await.map_err(map_error)?;

    Ok(Json(AuthUserDto {
        id: user.id.0.clone(),
        discord_id: user.discord_user_id.0.clone(),
        username: user.username.0.clone(),
        avatar: user.avatar_url.unwrap_or_default(),
        email: user.email.clone(),
        is_admin: user.permissions.contains(&"admin".to_string()),
        banned: user.banned,
    }))
}

#[utoipa::path(
    patch,
    path = "/api/v1/admin/users/{id}/role",
    request_body = UpdateUserRoleDto,
    params(
        ("id" = String, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "User role updated", body = AuthUserDto)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_user_role(
    State(service): State<Arc<Service>>,
    AdminUser(_admin_id): AdminUser,
    Path(id): Path<String>,
    Json(payload): Json<UpdateUserRoleDto>,
) -> Result<Json<AuthUserDto>, (axum::http::StatusCode, String)> {
    let user_id = UserId::from_str(&id).map_err(|_| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            "Invalid user ID".to_string(),
        )
    })?;

    // In this system, "role" update currently means updating permissions.
    // If the role is "admin", we add "admin" to permissions.
    // If it's "user", we remove "admin" from permissions.
    let mut permissions = vec![];
    if payload.role == "admin" {
        permissions.push("admin".to_string());
    }

    let user = service
        .auth
        .update_user_permissions(user_id, permissions)
        .await
        .map_err(map_error)?;

    Ok(Json(AuthUserDto {
        id: user.id.0.clone(),
        discord_id: user.discord_user_id.0.clone(),
        username: user.username.0.clone(),
        avatar: user.avatar_url.unwrap_or_default(),
        email: user.email.clone(),
        is_admin: user.permissions.contains(&"admin".to_string()),
        banned: user.banned,
    }))
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

#[utoipa::path(
    get,
    path = "/api/v1/admin/users/{id}/settings",
    params(
        ("id" = String, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "User settings (User scope)", body = PartialUserSettings)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_user_settings(
    State(service): State<Arc<Service>>,
    AdminUser(_admin_id): AdminUser,
    Path(id): Path<String>,
) -> Result<Json<PartialUserSettings>, (axum::http::StatusCode, String)> {
    let user_id = UserId::from_str(&id).map_err(|_| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            "Invalid user ID".to_string(),
        )
    })?;

    let settings = service
        .user_settings
        .get_settings(user_id)
        .await
        .map_err(map_error)?;

    Ok(Json(settings))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/users/{id}/settings/guilds/{guild_id}",
    params(
        ("id" = String, Path, description = "User ID"),
        ("guild_id" = String, Path, description = "Discord guild ID"),
    ),
    responses(
        (status = 200, description = "Guild-user settings override", body = PartialUserSettings)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_user_guild_settings(
    State(service): State<Arc<Service>>,
    AdminUser(_admin_id): AdminUser,
    Path((id, guild_id)): Path<(String, String)>,
) -> Result<Json<PartialUserSettings>, (axum::http::StatusCode, String)> {
    let user_id = UserId::from_str(&id).map_err(|_| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            "Invalid user ID".to_string(),
        )
    })?;

    let settings = service
        .user_settings
        .get_guild_user_settings(&user_id, &guild_id)
        .await
        .map_err(map_error)?
        .unwrap_or_default();

    Ok(Json(settings))
}

#[utoipa::path(
    put,
    path = "/api/v1/admin/users/{id}/settings",
    request_body = PartialUserSettings,
    params(
        ("id" = String, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "Updated user settings (User scope)", body = PartialUserSettings)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_user_settings(
    State(service): State<Arc<Service>>,
    AdminUser(_admin_id): AdminUser,
    Path(id): Path<String>,
    Json(body): Json<PartialUserSettings>,
) -> Result<Json<PartialUserSettings>, (axum::http::StatusCode, String)> {
    let user_id = UserId::from_str(&id).map_err(|_| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            "Invalid user ID".to_string(),
        )
    })?;

    let settings = service
        .user_settings
        .save_settings(user_id, body)
        .await
        .map_err(map_error)?;

    Ok(Json(settings))
}

#[utoipa::path(
    put,
    path = "/api/v1/admin/users/{id}/settings/guilds/{guild_id}",
    request_body = PartialUserSettings,
    params(
        ("id" = String, Path, description = "User ID"),
        ("guild_id" = String, Path, description = "Discord guild ID"),
    ),
    responses(
        (status = 200, description = "Updated guild-user settings override", body = PartialUserSettings)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_user_guild_settings(
    State(service): State<Arc<Service>>,
    AdminUser(_admin_id): AdminUser,
    Path((id, guild_id)): Path<(String, String)>,
    Json(body): Json<PartialUserSettings>,
) -> Result<Json<PartialUserSettings>, (axum::http::StatusCode, String)> {
    let user_id = UserId::from_str(&id).map_err(|_| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            "Invalid user ID".to_string(),
        )
    })?;

    let settings = service
        .user_settings
        .save_guild_user_settings(&user_id, &guild_id, body)
        .await
        .map_err(map_error)?;

    Ok(Json(settings))
}

pub async fn get_platform_stats(
    State(state): State<Arc<Service>>,
    AdminUser(_): AdminUser,
) -> Result<Json<PlatformStatsDto>, (axum::http::StatusCode, String)> {
    let global_unique_users = state
        .tap_metrics
        .get_global_unique_users()
        .await
        .map_err(|e| {
            let core_error = CoreError::StateError(e);
            map_error(core_error)
        })?;
    Ok(Json(PlatformStatsDto { global_unique_users }))
}
