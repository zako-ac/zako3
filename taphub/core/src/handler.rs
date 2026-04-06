use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use hex;
use protofish2::Timestamp;
use protofish2::mani::transfer::recv::TransferReliableRecvStream;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use zako3_preload_cache::{AudioCache, NextFrame, PreloadId, PreloadReadEndAction};
use zako3_taphub_transport_server::TapHubBridgeHandler;
use zako3_types::{
    AudioCachePolicy, AudioCacheType, AudioMetaResponse, AudioRequest, AudioRequestString,
    CachedAudioRequest,
    cache::{AudioCacheItem, AudioCacheItemKey},
    hq::{TapId, UserId},
};

use crate::hub::TapHub;

#[async_trait]
impl TapHubBridgeHandler for TapHub {
    async fn handle_request_audio(
        &self,
        request: CachedAudioRequest,
    ) -> Result<(AudioMetaResponse, mpsc::Receiver<(Timestamp, Bytes)>), String> {
        // Permission check
        let tap_id = self
            .state_service
            .get_tap_id_by_name(&request.tap_name)
            .await
            .map_err(|e| format!("Failed to get tap id: {}", e))?
            .ok_or_else(|| "Tap disconnected or not found".to_string())?;

        // Metrics
        if let Err(e) = self.metrics_service.inc_total_uses(tap_id.clone()).await {
            tracing::warn!(%e, "Failed to increment total_uses metric");
        }
        if let Err(e) = self
            .metrics_service
            .record_unique_user(tap_id.clone(), UserId(request.discord_user_id.0.clone()))
            .await
        {
            tracing::warn!(%e, "Failed to record unique_user metric");
        }

        let tap = self
            .app
            .hq_repository
            .get_tap_by_id(&tap_id.0.to_string())
            .await
            .ok_or_else(|| "Tap metadata not found".to_string())?;

        self.verify_permission(&tap, &request.discord_user_id)
            .await?;

        // Try cache hit
        let cache_item =
            build_cache_item(tap_id.clone(), &request.cache_key, &request.audio_request);

        if let Some(ref item) = cache_item {
            if let Some(entry) = self.audio_cache.get_entry(&item.tap_id, &item.key).await {
                tracing::info!("Cache hit for tap_id={}, key={}", item.tap_id.0, item.key);

                if let Some(reader) = self.audio_cache.open_reader(&item.tap_id, &item.key).await {
                    let (tx, rx) = mpsc::channel(100);
                    tokio::spawn(async move {
                        let mut reader = reader;
                        let mut frame_count = 0u64;
                        loop {
                            match reader.next_frame().await {
                                Ok(NextFrame::Frame(bytes)) => {
                                    let ts = Timestamp(frame_count * 20);
                                    frame_count += 1;
                                    if tx.send((ts, bytes)).await.is_err() {
                                        break;
                                    }
                                }
                                // Cache files are fully written — Pending cannot occur.
                                Ok(NextFrame::Pending) | Ok(NextFrame::Done) | Err(_) => break,
                            }
                        }
                    });
                    let meta = AudioMetaResponse {
                        metadatas: entry.metadatas,
                        cache_key: entry.cache_key,
                    };
                    return Ok((meta, rx));
                } else {
                    tracing::warn!(
                        "Cache entry found but failed to open reader for tap_id={}, key={}",
                        item.tap_id.0,
                        item.key
                    );
                }
            }
        }

        tracing::info!(
            "Cache miss for tap_id={}, cache_key={:?}",
            tap_id.0,
            cache_item.as_ref().map(|i| &i.key)
        );

        // Cache miss: request from zakofish
        let states = self
            .state_service
            .get_tap_states(&tap_id)
            .await
            .map_err(|e| format!("Failed to get tap states: {}", e))?;

        let connection_id = self
            .sampler
            .lock()
            .next_connection_id(&states)
            .ok_or_else(|| "No available connections for this tap".to_string())?;

        let (succ, rel, mut unrel) = self
            .zf_hub
            .request_audio(
                tap_id.clone(),
                connection_id,
                request.audio_request.clone(),
                Default::default(),
            )
            .await
            .map_err(|e| format!("Failed to request audio from hub: {}", e))?;

        // Cache _rel
        let rel_rx = bridge_rel(rel);
        if let Some(item) = cache_item {
            let cache = Arc::clone(&self.audio_cache);
            let metadatas = succ.metadatas.clone();
            let cache_key = succ.cache.clone();
            tokio::spawn(async move {
                if let Err(e) = cache.store(item, metadatas, cache_key, rel_rx).await {
                    tracing::warn!(%e, "Failed to store audio in cache");
                }
            });
        }

        let meta = AudioMetaResponse {
            metadatas: succ.metadatas,
            cache_key: succ.cache,
        };

        let (tx, rx) = mpsc::channel(100);
        tokio::spawn(async move {
            while let Some(chunk) = unrel.recv().await {
                if let Err(e) = tx.send((chunk.timestamp, chunk.content)).await {
                    tracing::warn!(%e, "Failed to send audio chunk to channel");
                    return;
                }
            }
        });

        Ok((meta, rx))
    }

    async fn handle_preload_audio(
        &self,
        req: CachedAudioRequest,
    ) -> Result<AudioMetaResponse, String> {
        // Permission check
        let tap_id = self
            .state_service
            .get_tap_id_by_name(&req.tap_name)
            .await
            .map_err(|e| format!("Failed to get tap id: {}", e))?
            .ok_or_else(|| "Tap disconnected or not found".to_string())?;

        let tap = self
            .app
            .hq_repository
            .get_tap_by_id(&tap_id.0.to_string())
            .await
            .ok_or_else(|| "Tap metadata not found".to_string())?;

        self.verify_permission(&tap, &req.discord_user_id).await?;

        // Connection selection
        let states = self
            .state_service
            .get_tap_states(&tap_id)
            .await
            .map_err(|e| format!("Failed to get tap states: {}", e))?;

        let connection_id = self
            .sampler
            .lock()
            .next_connection_id(&states)
            .ok_or_else(|| "No available connections for this tap".to_string())?;

        // Request audio from zakofish — only _rel is used for preloading
        let (succ, rel, _unrel) = self
            .zf_hub
            .request_audio(
                tap_id.clone(),
                connection_id,
                req.audio_request.clone(),
                Default::default(),
            )
            .await
            .map_err(|e| format!("Failed to request audio from hub: {}", e))?;

        // Preload _rel to disk
        let preload_id = PreloadId(uuid::Uuid::new_v4().as_u128() as u64);
        let rel_rx = bridge_rel(rel);
        self.audio_preload.preload(preload_id, rel_rx);

        // Determine finalization action
        let action = match build_cache_item(tap_id, &succ.cache, &req.audio_request) {
            Some(item) => PreloadReadEndAction::MoveToCache {
                item,
                metadatas: succ.metadatas.clone(),
                cache_key: succ.cache.clone(),
                cache: Arc::clone(&self.audio_cache) as Arc<dyn AudioCache>,
            },
            None => PreloadReadEndAction::Delete,
        };

        // Spawn finalization task: drain preload, then move to cache or delete
        let audio_preload = Arc::clone(&self.audio_preload);
        tokio::spawn(async move {
            // Wait for preload file to appear (write task creates it)
            let mut reader = loop {
                match audio_preload.open_reader(preload_id).await {
                    Some(r) => break r,
                    None => tokio::time::sleep(Duration::from_millis(10)).await,
                }
            };
            // Drain frames until Done
            loop {
                match reader.next_frame().await {
                    Ok(NextFrame::Frame(_)) => {}
                    Ok(NextFrame::Pending) => {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    Ok(NextFrame::Done) => {
                        if let Err(e) = reader.finalize(preload_id, &audio_preload, action).await {
                            tracing::warn!(%e, "Failed to finalize preload");
                        }
                        break;
                    }
                    Err(e) => {
                        tracing::warn!(%e, "Preload read error during finalization");
                        break;
                    }
                }
            }
        });

        Ok(AudioMetaResponse {
            metadatas: succ.metadatas,
            cache_key: succ.cache,
        })
    }

    async fn handle_request_audio_meta(
        &self,
        req: AudioRequest,
    ) -> Result<AudioMetaResponse, String> {
        let tap_id = self
            .state_service
            .get_tap_id_by_name(&req.tap_name)
            .await
            .map_err(|e| format!("Failed to get tap id: {}", e))?
            .ok_or_else(|| "Tap disconnected or not found".to_string())?;

        let tap = self
            .app
            .hq_repository
            .get_tap_by_id(&tap_id.0.to_string())
            .await
            .ok_or_else(|| "Tap metadata not found".to_string())?;

        self.verify_permission(&tap, &req.discord_user_id).await?;

        let states = self
            .state_service
            .get_tap_states(&tap_id)
            .await
            .map_err(|e| format!("Failed to get tap states: {}", e))?;

        let connection_id = self
            .sampler
            .lock()
            .next_connection_id(&states)
            .ok_or_else(|| "No available connections for this tap".to_string())?;

        let meta = self
            .zf_hub
            .request_audio_metadata(tap_id, connection_id, req.request, Default::default())
            .await
            .map_err(|e| format!("Failed to request audio metadata: {}", e))?;

        Ok(AudioMetaResponse {
            metadatas: meta.metadatas,
            cache_key: meta.cache,
        })
    }
}

impl TapHub {
    async fn verify_permission(
        &self,
        tap: &zako3_types::hq::Tap,
        discord_user_id: &zako3_types::hq::DiscordUserId,
    ) -> Result<(), String> {
        use zako3_types::hq::TapPermission;

        match &tap.permission {
            TapPermission::Public => Ok(()),
            TapPermission::OwnerOnly => {
                let user = self
                    .app
                    .hq_repository
                    .get_user_by_discord_id(discord_user_id)
                    .await
                    .ok_or_else(|| "User not found in HQ".to_string())?;

                if user.id == tap.owner_id {
                    Ok(())
                } else {
                    Err("User is not the owner of this tap".to_string())
                }
            }
            TapPermission::Whitelisted { user_ids } => {
                if user_ids.contains(&discord_user_id.0) {
                    Ok(())
                } else {
                    Err("User is not whitelisted for this tap".to_string())
                }
            }
            TapPermission::Blacklisted { user_ids } => {
                if user_ids.contains(&discord_user_id.0) {
                    Err("User is blacklisted for this tap".to_string())
                } else {
                    Ok(())
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build an `AudioCacheItem` from a request's cache policy.
/// Returns `None` for `AudioCacheType::None` (no caching).
fn build_cache_item(
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

/// Bridge a reliable stream to an `mpsc::Receiver<Bytes>`.
/// Each `recv()` on the reliable stream returns a batch of chunks.
fn bridge_rel(mut rel: TransferReliableRecvStream) -> mpsc::Receiver<Bytes> {
    let (tx, rx) = mpsc::channel(100);
    tokio::spawn(async move {
        while let Some(chunks) = rel.recv().await {
            for chunk in chunks {
                if tx.send(chunk.content).await.is_err() {
                    return;
                }
            }
        }
    });
    rx
}
