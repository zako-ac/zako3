use zako3_preload_cache::AudioCache;
use zako3_types::CachedAudioRequest;

use crate::hub::TapHub;

use super::cache::build_cache_item;

pub(crate) async fn handle_invalidate_cache_inner(
    tap_hub: &TapHub,
    request: CachedAudioRequest,
) -> Result<(), String> {
    let tap_id = request.tap_id.clone();

    let Some(item) =
        build_cache_item(tap_id.clone(), &request.cache_key, &request.audio_request)
    else {
        return Ok(());
    };

    tap_hub
        .audio_cache
        .delete(&item.tap_id, &item.key)
        .await
        .map_err(|e| format!("Failed to delete cache: {}", e))?;

    tracing::warn!(
        tap_id = %tap_id.0,
        key = %item.key,
        "Cache invalidated due to client decode failure"
    );

    Ok(())
}
