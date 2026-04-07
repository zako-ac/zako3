use chrono::Utc;
use hex;
use sha2::{Digest, Sha256};
use zako3_preload_cache::AudioCache;
use zako3_types::{
    AudioCachePolicy, AudioCacheType, AudioRequestString,
    cache::{AudioCacheItem, AudioCacheItemKey},
    hq::TapId,
};

use crate::hub::TapHub;

/// Build an `AudioCacheItem` from a request's cache policy.
/// Returns `None` for `AudioCacheType::None` (no caching).
pub(crate) fn build_cache_item(
    tap_id: TapId,
    policy: &AudioCachePolicy,
    ars: &AudioRequestString,
) -> Option<AudioCacheItem> {
    let expire_at = policy
        .ttl_seconds
        .map(|ttl| Utc::now() + chrono::Duration::seconds(ttl as i64));

    let key = match &policy.cache_type {
        AudioCacheType::None => return None,
        AudioCacheType::ARHash => {
            let hash = hex::encode(Sha256::digest(ars.to_string().as_bytes()));
            AudioCacheItemKey::ARHash(hash)
        }
        AudioCacheType::CacheKey(k) => AudioCacheItemKey::CacheKey(k.clone()),
    };

    Some(AudioCacheItem {
        key,
        tap_id,
        expire_at,
    })
}

/// Resolve metadata from either inline Metadatas or the UseCached fallback.
/// The fallback checks the cache for an ARHash key derived from the audio request.
pub(crate) async fn resolve_metadata(
    tap_hub: &TapHub,
    metadatas: zakofish::types::message::AttachedMetadata,
    tap_id: &TapId,
    audio_request_str: &str,
) -> Vec<zako3_types::AudioMetadata> {
    use zakofish::types::message::AttachedMetadata;

    match metadatas {
        AttachedMetadata::Metadatas(v) => v,
        AttachedMetadata::UseCached => {
            let meta_hash = hex::encode(Sha256::digest(audio_request_str.as_bytes()));
            let meta_key = AudioCacheItemKey::ARHash(meta_hash);
            tap_hub
                .audio_cache
                .get_entry(tap_id, &meta_key)
                .await
                .map(|e| e.metadatas)
                .unwrap_or_else(|| {
                    tracing::warn!(
                        "UseCached metadata requested but no cache entry found for tap_id={}",
                        tap_id.0
                    );
                    vec![]
                })
        }
    }
}
