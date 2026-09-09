//! Serving audio through HQ's gateway instead of taphub.
//!
//! Implements the same [`TapHubService`] the engine already calls, so the
//! decoder, mixer and songbird path are untouched — the only thing that changes
//! is where the frames come from.
//!
//! The ordering that matters: the receiver is armed **before** HQ is called.
//! HQ may reach the tap and the tap may start sending while the RPC is still in
//! flight, so anything else would need a handshake to close the gap.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use protofish4::{Endpoint, ReceiverConfig, RelOutcome, SessionKey};
use tokio::sync::mpsc;
use zako3_audio_engine_core::error::{ZakoError, ZakoResult};
use zako3_audio_engine_core::service::taphub::TapHubService;
use zako3_audio_engine_core::types::{
    AudioMetaResponse, AudioRequest, AudioResponse, CachedAudioRequest,
};
use zako3_cache_client::RemoteAudioCache;
use zako3_opus_jitter::{JitterConfig, OpusJitterBuffer, TimedFrame};
use zako3_types::TapHubError;
use zako3_types::cache::AudioCacheItem;
use zako3_types::hq::audio_dispatch::{AudioDispatch, MetaDispatch, SinkTicket, StreamReport};

use crate::cache_tee::CacheTeeFactory;
use crate::hq_client::HqAudioClient;

/// PCM frames buffered towards the mixer.
const PCM_QUEUE: usize = 100;

/// Frames buffered between the UDP receiver and the jitter buffer.
const INGEST_QUEUE: usize = 256;

pub struct HqAudioService {
    hq: Arc<HqAudioClient>,
    endpoint: Arc<Endpoint>,
    tee: CacheTeeFactory,
    cache: Arc<RemoteAudioCache>,
    /// This engine's id, as advertised in the sink registry. HQ resolves it to
    /// an address when filling `deliver_to`.
    sink_id: String,
    /// Where the engine falls back for taps that have not been migrated.
    legacy: Arc<dyn TapHubService>,
}

impl HqAudioService {
    pub fn new(
        hq: Arc<HqAudioClient>,
        endpoint: Arc<Endpoint>,
        cache: Arc<RemoteAudioCache>,
        sink_id: String,
        legacy: Arc<dyn TapHubService>,
    ) -> Self {
        Self {
            hq,
            endpoint,
            tee: CacheTeeFactory::new(Arc::clone(&cache)),
            cache,
            sink_id,
            legacy,
        }
    }

    /// Tees that ended without committing, for the metrics endpoint.
    pub fn cache_tee_aborts(&self) -> u64 {
        self.tee.aborted_count()
    }

    /// Play an already-cached entry, straight from the cache server over HTTP.
    ///
    /// No tap and no UDP: this is the reliable path, and it is what makes a
    /// second play of the same track cost nothing.
    async fn play_from_cache(
        &self,
        meta: AudioMetaResponse,
        item: AudioCacheItem,
    ) -> ZakoResult<AudioResponse> {
        use zako3_preload_cache::{AudioCache, NextFrame};

        let mut reader = self
            .cache
            .open_reader(&item.tap_id, &item.key)
            .await
            .ok_or_else(|| {
                ZakoError::TapHub(TapHubError::Internal(
                    "HQ reported a cache hit but the entry could not be opened".to_string(),
                ))
            })?;

        let (frame_tx, frame_rx) = mpsc::channel(INGEST_QUEUE);
        tokio::spawn(async move {
            // Frames come out in order, so timestamps follow the frame index.
            let mut index = 0u64;
            loop {
                match reader.next_frame().await {
                    Ok(NextFrame::Frame(bytes)) => {
                        let ts_ms = index * zako3_opus_jitter::FRAME_MS;
                        index += 1;
                        if frame_tx
                            .send(TimedFrame { ts_ms, payload: bytes.to_vec() })
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                    Ok(NextFrame::Pending) => continue,
                    Ok(NextFrame::Done) => return,
                    Err(e) => {
                        tracing::warn!(%e, "cache read failed mid-stream");
                        return;
                    }
                }
            }
        });

        Ok(AudioResponse {
            cache_key: Some(meta.cache_key),
            metadatas: meta.metadatas,
            stream: self.spawn_decoder(frame_rx, None),
        })
    }

    /// Decode frames into PCM for the mixer.
    ///
    /// The cache tee, when there is one, is fed from the reliable side rather
    /// than from here — playback and caching share a source but never a
    /// deadline.
    fn spawn_decoder(
        &self,
        frames: mpsc::Receiver<TimedFrame>,
        request_id: Option<uuid::Uuid>,
    ) -> mpsc::Receiver<Vec<f32>> {
        let (pcm_tx, pcm_rx) = mpsc::channel(PCM_QUEUE);
        let hq = Arc::clone(&self.hq);

        tokio::spawn(async move {
            let mut jitter = match OpusJitterBuffer::new(frames, JitterConfig::default()) {
                Ok(j) => j,
                Err(e) => {
                    tracing::error!(%e, "failed to create the jitter buffer");
                    return;
                }
            };

            loop {
                match jitter.yield_pcm().await {
                    Ok(Some(pcm)) => {
                        if pcm_tx.send(pcm).await.is_err() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        tracing::error!(%e, "jitter buffer error");
                        break;
                    }
                }
            }

            if jitter.dropped_frames() > 0 {
                // A sender outrunning playback, rather than a network problem.
                tracing::warn!(
                    dropped = jitter.dropped_frames(),
                    ?request_id,
                    "frames dropped: the sender is running ahead of playback"
                );
            }
            let _ = hq;
        });

        pcm_rx
    }
}

#[async_trait]
impl TapHubService for HqAudioService {
    async fn request_audio(&self, request: CachedAudioRequest) -> ZakoResult<AudioResponse> {
        // Armed first, and held until the transfer is set up: the guard
        // disarms on drop, so an error anywhere below cannot leak a slot.
        let raw_key = protofish4::random_key();
        let ticket = SinkTicket {
            request_id: uuid::Uuid::new_v4(),
            encryption_key: raw_key,
        };
        let key = SessionKey::from_bytes(&raw_key)
            .map_err(|e| ZakoError::TapHub(TapHubError::Internal(e.to_string())))?;

        let (armed, streams) = self
            .endpoint
            .arm(
                protofish4::RequestId(ticket.request_id),
                key,
                ReceiverConfig::audio_engine(),
            )
            .await
            .map_err(|e| ZakoError::TapHub(TapHubError::Internal(e.to_string())))?;

        let dispatch = self
            .hq
            .request_audio(&self.sink_id, ticket.clone(), request.clone())
            .await?;

        match dispatch {
            AudioDispatch::Legacy => {
                drop(armed);
                self.legacy.request_audio(request).await
            }
            AudioDispatch::CacheHit { meta, item } => {
                drop(armed);
                self.play_from_cache(meta, item).await
            }
            AudioDispatch::Dispatched { meta, cache_item } => {
                let tee = self.tee.open(
                    cache_item,
                    meta.metadatas.clone(),
                    meta.cache_key.clone(),
                );

                let (frame_tx, frame_rx) = mpsc::channel(INGEST_QUEUE);
                let mut streams = streams;
                let request_id = ticket.request_id;
                let hq = Arc::clone(&self.hq);

                // Playback: every frame that arrives, in arrival order.
                tokio::spawn(async move {
                    // Held for the life of the transfer; dropping it here would
                    // disarm the receiver mid-stream.
                    let _armed = armed;
                    while let Some(frame) = streams.unrel.recv().await {
                        if frame_tx
                            .send(TimedFrame {
                                ts_ms: frame.ts.0,
                                payload: frame.payload,
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                });

                // Caching: the recovered copy, on its own task with no way to
                // signal back into playback.
                tokio::spawn(async move {
                    let mut received = 0u64;
                    while let Some(frame) = streams.rel.recv().await {
                        received += 1;
                        tee.offer(Bytes::from(frame.payload));
                    }

                    // What HQ hears from *this* end. The tap reports its own
                    // count over the WebSocket, and the two disagreeing is
                    // exactly the signal worth having — HQ sees neither the
                    // audio nor the loss otherwise.
                    let report = match streams.outcome.await {
                        Ok(RelOutcome::Complete { .. }) => {
                            tee.commit();
                            StreamReport::Completed { frames_sent: received }
                        }
                        Ok(RelOutcome::Aborted(reason)) => {
                            tracing::info!(?reason, %request_id, "cache copy abandoned");
                            // Dropping the tee aborts the upload, which is the
                            // safe default: an incomplete entry must never be
                            // committed.
                            StreamReport::Aborted {
                                frames_sent: received,
                                reason: format!("{reason:?}"),
                            }
                        }
                        Err(_) => StreamReport::Aborted {
                            frames_sent: received,
                            reason: "receiver closed before reporting".to_string(),
                        },
                    };
                    hq.report_stream_outcome(request_id, report).await;
                });

                Ok(AudioResponse {
                    cache_key: Some(meta.cache_key),
                    metadatas: meta.metadatas,
                    stream: self.spawn_decoder(frame_rx, Some(request_id)),
                })
            }
        }
    }

    async fn preload_audio(&self, request: CachedAudioRequest) -> ZakoResult<AudioMetaResponse> {
        match self.hq.preload_audio(request.clone()).await? {
            MetaDispatch::Ready(meta) => Ok(meta),
            MetaDispatch::Legacy => self.legacy.preload_audio(request).await,
        }
    }

    async fn request_audio_meta(&self, request: AudioRequest) -> ZakoResult<AudioMetaResponse> {
        match self.hq.request_audio_meta(request.clone()).await? {
            MetaDispatch::Ready(meta) => Ok(meta),
            MetaDispatch::Legacy => self.legacy.request_audio_meta(request).await,
        }
    }
}

/// Advertise this engine's UDP address, so HQ can point taps at it.
///
/// Refreshed well inside the lease so two consecutive failures do not make a
/// healthy engine look gone.
pub fn spawn_sink_heartbeat(
    registry: zako3_states::SinkRegistry,
    ad: zako3_states::SinkAdvertisement,
) {
    let interval = Duration::from_secs((registry.lease_ttl_secs() / 3).max(1));
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if let Err(e) = registry.advertise(&ad).await {
                tracing::warn!(%e, sink_id = %ad.sink_id, "failed to advertise this sink");
            }
        }
    });
}
