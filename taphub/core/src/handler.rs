use async_trait::async_trait;
use bytes::Bytes;
use protofish2::Timestamp;
use tokio::sync::mpsc;
use zako3_taphub_transport_server::TapHubBridgeHandler;
use zako3_types::{
    AudioCachePolicy, AudioCacheType, AudioMetaResponse, AudioMetadata, AudioRequest,
    CachedAudioRequest, hq::UserId,
};

use crate::hub::TapHub;

#[async_trait]
impl TapHubBridgeHandler for TapHub {
    async fn handle_request_audio(
        &self,
        request: CachedAudioRequest,
    ) -> Result<(AudioMetaResponse, mpsc::Receiver<(Timestamp, Bytes)>), String> {
        // Permission Check
        let tap_id_opt = self
            .state_service
            .get_tap_id_by_name(&request.tap_name)
            .await
            .map_err(|e| format!("Failed to get tap id: {}", e))?;
        let tap_id = tap_id_opt.ok_or_else(|| "Tap disconnected or not found".to_string())?;

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

        let (tx, rx) = mpsc::channel(100);

        let states = &self
            .state_service
            .get_tap_states(&tap_id)
            .await
            .map_err(|e| format!("Failed to get tap states: {}", e))?;

        let connection_id = self
            .sampler
            .lock()
            .next_connection_id(states)
            .ok_or_else(|| "No available connections for this tap".to_string())?;

        let (succ, _rel, mut unrel) = self
            .zf_hub
            .request_audio(
                tap_id.clone(),
                connection_id,
                request.audio_request,
                Default::default(),
            )
            .await
            .map_err(|e| format!("Failed to request audio from hub: {}", e))?;

        // TODO: cache rel

        let meta = AudioMetaResponse {
            metadatas: succ.metadatas,
            cache_key: succ.cache,
        };

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
        _req: CachedAudioRequest,
    ) -> Result<AudioMetaResponse, String> {
        Ok(AudioMetaResponse {
            metadatas: vec![AudioMetadata::Title("Dummy Title".to_string())],
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
        })
    }

    async fn handle_request_audio_meta(
        &self,
        req: AudioRequest,
    ) -> Result<AudioMetaResponse, String> {
        let tap_id_opt = self
            .state_service
            .get_tap_id_by_name(&req.tap_name)
            .await
            .map_err(|e| format!("Failed to get tap id: {}", e))?;
        let tap_id = tap_id_opt.ok_or_else(|| "Tap disconnected or not found".to_string())?;

        let tap = self
            .app
            .hq_repository
            .get_tap_by_id(&tap_id.0.to_string())
            .await
            .ok_or_else(|| "Tap metadata not found".to_string())?;

        self.verify_permission(&tap, &req.discord_user_id).await?;

        Ok(AudioMetaResponse {
            metadatas: vec![AudioMetadata::Title("Dummy Title".to_string())],
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
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
