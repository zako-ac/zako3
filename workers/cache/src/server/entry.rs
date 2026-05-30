use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use zako3_cache_client::{
    CacheEntryDto, ClearTapResp, DeleteEntryResp, EntryQuery, StoreMetadataReq, TapQuery,
};
use zako3_preload_cache::AudioCache;
use zako3_types::{cache::AudioCacheItemKey, hq::TapId};

use super::state::AppState;

/// `GET /entry?tap_id&key` — return the cache entry as JSON, or 404.
pub async fn get_entry(
    State(state): State<AppState>,
    Query(q): Query<EntryQuery>,
) -> Result<Json<CacheEntryDto>, StatusCode> {
    let key: AudioCacheItemKey =
        serde_json::from_str(&q.key).map_err(|_| StatusCode::BAD_REQUEST)?;
    let tap_id = TapId(q.tap_id);
    let entry = state
        .cache
        .get_entry(&tap_id, &key)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(entry.into()))
}

/// `DELETE /entry?tap_id&key` — remove a cache entry. Reports whether a matching
/// entry existed via `{ "deleted": bool }`.
pub async fn delete_entry(
    State(state): State<AppState>,
    Query(q): Query<EntryQuery>,
) -> Result<Json<DeleteEntryResp>, StatusCode> {
    let key: AudioCacheItemKey =
        serde_json::from_str(&q.key).map_err(|_| StatusCode::BAD_REQUEST)?;
    let tap_id = TapId(q.tap_id);
    let deleted = state
        .cache
        .delete_returning_found(&tap_id, &key)
        .await
        .map_err(|e| {
            tracing::warn!(%e, "delete failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(DeleteEntryResp { deleted }))
}

/// `DELETE /entries?tap_id` — remove every cache entry for a tap.
pub async fn delete_entries(
    State(state): State<AppState>,
    Query(q): Query<TapQuery>,
) -> Result<Json<ClearTapResp>, StatusCode> {
    let tap_id = TapId(q.tap_id);
    let deleted = state
        .cache
        .delete_all_for_tap(&tap_id)
        .await
        .map_err(|e| {
            tracing::warn!(%e, "delete_all_for_tap failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(ClearTapResp { deleted }))
}

/// `POST /metadata` — write a metadata-only entry (no audio file).
pub async fn store_metadata(
    State(state): State<AppState>,
    Json(req): Json<StoreMetadataReq>,
) -> Result<StatusCode, StatusCode> {
    state
        .cache
        .store_metadata(req.item, req.metadatas, req.cache_key)
        .await
        .map_err(|e| {
            tracing::warn!(%e, "store_metadata failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(StatusCode::NO_CONTENT)
}
