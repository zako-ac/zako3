use crate::middleware::auth::AdminUser;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use hq_core::Service;
use hq_types::{cache::AudioCacheItemKey, hq::TapId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// Request body for deleting a single cached audio entry. Provide exactly one of
/// `audio_request` (hashed to an ARHash key) or `cache_key` (used as-is).
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct DeleteCacheEntryDto {
    /// Raw audio request string; hashed to `ARHash(hex(sha256(request)))`.
    pub audio_request: Option<String>,
    /// Explicit cache key, used as `CacheKey(..)`.
    pub cache_key: Option<String>,
}

/// Response with the number of cache entries removed.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ClearCacheResponseDto {
    pub deleted: usize,
}

/// Response indicating whether a matching cache entry was found and deleted.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DeleteCacheEntryResultDto {
    pub found: bool,
}

/// `DELETE /api/v1/admin/taps/{id}/cache` — clear every cached entry for a tap.
#[utoipa::path(
    delete,
    path = "/api/v1/admin/taps/{id}/cache",
    params(("id" = String, Path, description = "Tap ID")),
    responses((status = 200, description = "Number of entries removed", body = ClearCacheResponseDto)),
    security(("bearer_auth" = []))
)]
pub async fn clear_tap_cache(
    State(service): State<Arc<Service>>,
    AdminUser(_admin_id): AdminUser,
    Path(tap_id): Path<String>,
) -> Result<Json<ClearCacheResponseDto>, (StatusCode, String)> {
    let deleted = service
        .cache_admin
        .delete_all_for_tap(&TapId(tap_id))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ClearCacheResponseDto { deleted }))
}

/// `DELETE /api/v1/admin/taps/{id}/cache/entry` — delete one cached audio entry.
#[utoipa::path(
    delete,
    path = "/api/v1/admin/taps/{id}/cache/entry",
    params(("id" = String, Path, description = "Tap ID")),
    request_body = DeleteCacheEntryDto,
    responses((status = 200, description = "Whether a matching entry was found and deleted", body = DeleteCacheEntryResultDto)),
    security(("bearer_auth" = []))
)]
pub async fn delete_tap_cache_entry(
    State(service): State<Arc<Service>>,
    AdminUser(_admin_id): AdminUser,
    Path(tap_id): Path<String>,
    Json(body): Json<DeleteCacheEntryDto>,
) -> Result<Json<DeleteCacheEntryResultDto>, (StatusCode, String)> {
    let key = if let Some(k) = body.cache_key.filter(|s| !s.trim().is_empty()) {
        AudioCacheItemKey::CacheKey(k)
    } else if let Some(req) = body.audio_request.filter(|s| !s.is_empty()) {
        let hash = hex::encode(Sha256::digest(req.as_bytes()));
        AudioCacheItemKey::ARHash(hash)
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            "either audio_request or cache_key is required".to_string(),
        ));
    };

    let found = service
        .cache_admin
        .delete_entry(&TapId(tap_id), &key)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(DeleteCacheEntryResultDto { found }))
}
