//! Serving audio requests.
//!
//! This is taphub's request path, moved into HQ and with the audio taken out of
//! it. HQ resolves the tap, checks permission, looks in the cache and — on a
//! miss — asks a tap to stream, but the audio itself goes straight from tap to
//! sink over UDP and never passes through here.
//!
//! Deliberately free of anything gateway-shaped: dispatch is a trait, so this
//! logic is testable without a WebSocket and the two could be split into
//! separate services later.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use sha2::Digest;
use uuid::Uuid;
use zako3_metrics::TapMetricsService;
use zako3_preload_cache::AudioCache;
use zako3_cache_client::{CreateIngestReq, FinalizeIngestReq, IngestCreatedResp};
use zako3_states::{RedisPubSub, SinkRegistry};
use hq_types::cache::{AudioCacheItem, AudioCacheItemKey};
use hq_types::hq::audio_dispatch::{AudioDispatch, MetaDispatch, SinkTicket, StreamReport};
use hq_types::hq::history::{PlayAudioHistory, UseHistoryEntry};
use hq_types::hq::{DiscordUserId, Tap, TapId};
use hq_types::{
    AudioCachePolicy, AudioCacheType, AudioMetaResponse, AudioMetadata, AudioRequest,
    AudioRequestString, CachedAudioRequest, TapHubError,
};
use zakofish4_common::messages::{
    AttachedMetadata, AudioMetadataRequestMessage, AudioRequestMessage, RequestVariant,
    ResponseVariant,
};
use zakofish4_common::model::{DeliverTo, EncryptionKey};
use zakofish4_common::state::PendingRequest;

use crate::repo::TapRepository;
use crate::service::TapService;

/// Why a dispatch did not reach a tap.
#[derive(Debug, Clone, thiserror::Error)]
pub enum DispatchFailure {
    #[error("tap has no connected instance")]
    NoConnection,
    #[error("the tap did not answer in time")]
    Timeout,
    #[error("the tap's connection closed before it answered")]
    Disconnected,
    #[error("{0}")]
    Transport(String),
}

/// Opening a UDP ingest slot on the cache worker.
///
/// A trait rather than a concrete client so the preload path stays testable
/// without a cache server, and so HQ never grows a second place that knows how
/// the cache is reached.
#[async_trait]
pub trait CacheIngest: Send + Sync + 'static {
    /// Mint a ticket and arm the cache worker's receiver. The returned
    /// `preload_id` must be finalized or aborted by the caller.
    async fn open(&self, req: CreateIngestReq) -> Result<IngestCreatedResp, String>;

    /// Attach the metadata the slot was opened without.
    async fn finalize(&self, preload_id: u64, req: FinalizeIngestReq) -> Result<(), String>;

    /// Discard a slot that will never be filled.
    async fn abort(&self, preload_id: u64);
}

#[async_trait]
impl CacheIngest for zako3_cache_client::RemoteAudioCache {
    async fn open(&self, req: CreateIngestReq) -> Result<IngestCreatedResp, String> {
        self.open_ingest(&req).await.map_err(|e| e.to_string())
    }

    async fn finalize(&self, preload_id: u64, req: FinalizeIngestReq) -> Result<(), String> {
        self.finalize_ingest(preload_id, &req)
            .await
            .map_err(|e| e.to_string())
    }

    async fn abort(&self, preload_id: u64) {
        if let Err(e) = self.abort_preload(preload_id).await {
            tracing::warn!(%e, preload_id, "failed to abort a cache ingest slot");
        }
    }
}

/// Aborts an ingest slot unless it is disarmed.
///
/// Every path out of a dispatch that fails — no connection, a refusal, a
/// timeout, a malformed answer — leaves a session open on the cache worker
/// that is already shadowing its cache key for `GET /stream`. Waiting for the
/// reaper means every read of that key tails a partial file that will never be
/// finished, for the length of the TTL. A guard rather than a cleanup call per
/// branch, because the branch that gets forgotten is the one that matters.
struct IngestGuard {
    ingest: Arc<dyn CacheIngest>,
    preload_id: Option<u64>,
    request_id: Option<Uuid>,
    sinks: SinkRegistry,
}

impl IngestGuard {
    fn disarm(&mut self) {
        self.preload_id = None;
    }
}

impl Drop for IngestGuard {
    fn drop(&mut self) {
        let Some(preload_id) = self.preload_id else {
            return;
        };
        let ingest = Arc::clone(&self.ingest);
        let sinks = self.sinks.clone();
        let request_id = self.request_id;
        tokio::spawn(async move {
            ingest.abort(preload_id).await;
            if let Some(id) = request_id {
                sinks.clear_route(&id).await;
            }
        });
    }
}

/// Getting a request to a tap. Implemented by the gateway.
#[async_trait]
pub trait TapDispatcher: Send + Sync + 'static {
    async fn dispatch(
        &self,
        tap_id: &TapId,
        pending: PendingRequest,
    ) -> Result<ResponseVariant, DispatchFailure>;
}

/// Timeouts for the control plane.
///
/// One place, because these used to be spread across five processes with no
/// owner: an inner deadline longer than its outer one means the outer fires
/// first and the specific error is replaced by a generic timeout.
#[derive(Debug, Clone)]
pub struct AudioTimeouts {
    /// AE → HQ → tap → response. Must stay below the audio engine's own
    /// per-attempt budget.
    pub dispatch: Duration,
    /// Metadata requests answer faster and are worth failing sooner.
    pub metadata: Duration,
    /// Preloads, which must answer inside the audio engine's own per-attempt
    /// HQ budget (`default_hq_request_timeout_ms`, 6s). If the caller gives up
    /// first, the RPC handler is cancelled and its `IngestGuard` aborts a
    /// transfer the tap is streaming perfectly well — so an inversion here does
    /// not merely report a spurious failure, it destroys good audio.
    pub preload: Duration,
}

impl Default for AudioTimeouts {
    fn default() -> Self {
        Self {
            dispatch: Duration::from_secs(10),
            metadata: Duration::from_secs(6),
            preload: Duration::from_secs(5),
        }
    }
}

#[derive(Clone)]
pub struct AudioRequestService {
    tap_repo: Arc<dyn TapRepository>,
    tap: TapService,
    cache: Arc<dyn AudioCache>,
    metrics: TapMetricsService,
    history: Arc<RedisPubSub>,
    sinks: SinkRegistry,
    dispatcher: Arc<dyn TapDispatcher>,
    /// Public address of the shared UDP proxy, while it owns the only public
    /// IP. `None` once every sink advertises its own.
    proxy_addr: Option<String>,
    /// The cache worker's ingest API, when the preload path is enabled.
    ingest: Option<Arc<dyn CacheIngest>>,
    /// Which sink id the cache worker advertises under. Configured rather than
    /// discovered, because a scan of the registry for "the one of kind cache"
    /// would be a lookup with no key.
    cache_sink_id: String,
    timeouts: AudioTimeouts,
}

impl AudioRequestService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tap_repo: Arc<dyn TapRepository>,
        tap: TapService,
        cache: Arc<dyn AudioCache>,
        metrics: TapMetricsService,
        history: Arc<RedisPubSub>,
        sinks: SinkRegistry,
        dispatcher: Arc<dyn TapDispatcher>,
        proxy_addr: Option<String>,
    ) -> Self {
        Self {
            tap_repo,
            tap,
            cache,
            metrics,
            history,
            sinks,
            dispatcher,
            proxy_addr,
            ingest: None,
            cache_sink_id: "cache".to_string(),
            timeouts: AudioTimeouts::default(),
        }
    }

    /// Enable the direct preload path: tap → cache worker, no audio engine.
    pub fn with_cache_ingest(mut self, ingest: Arc<dyn CacheIngest>, sink_id: String) -> Self {
        self.ingest = Some(ingest);
        self.cache_sink_id = sink_id;
        self
    }

    pub fn with_timeouts(mut self, timeouts: AudioTimeouts) -> Self {
        self.timeouts = timeouts;
        self
    }

    // --- audio -------------------------------------------------------------

    /// Serve an audio request.
    ///
    /// The caller has already armed `ticket`, so a tap may begin sending before
    /// this returns.
    pub async fn request_audio(
        &self,
        sink_id: &str,
        ticket: SinkTicket,
        req: CachedAudioRequest,
    ) -> Result<AudioDispatch, TapHubError> {
        let tap = self.resolve_tap(&req.tap_id).await?;
        self.verify_permission(&tap, &req.discord_user_id).await?;

        let cache_item = build_cache_item(&req.tap_id, &req.cache_key, &req.audio_request);

        if let Some(hit) = self.try_cache_hit(&tap, cache_item.as_ref()).await {
            self.record_use(&req, true, true).await;
            return Ok(AudioDispatch::CacheHit {
                meta: hit.0,
                item: hit.1,
            });
        }

        // Everything above is shared with the taphub path and safe to run for
        // any tap; only past here does the protocol actually diverge.
        if !tap.gateway_v4 {
            return Ok(AudioDispatch::Legacy);
        }

        if !ticket.looks_valid() {
            return Err(TapHubError::Internal(
                "audio engine sent a malformed sink ticket".to_string(),
            ));
        }

        let deliver_to = self.resolve_deliver_to(sink_id, &ticket).await?;

        let request_id = zakofish4_common::model::RequestId(ticket.request_id);
        let ars = req.audio_request.clone();
        let user = req.discord_user_id.clone();
        let headers = req.headers.clone();
        let key = EncryptionKey(ticket.encryption_key);
        let timeout = self.timeouts.dispatch;

        let response = self
            .dispatcher
            .dispatch(
                &req.tap_id,
                PendingRequest {
                    request_id,
                    variant: RequestVariant::AudioRequest(AudioRequestMessage {
                        ars: zakofish4_common::model::AudioRequestString(ars.to_string()),
                        discord_user_id: zakofish4_common::model::DiscordUserId(user.0.clone()),
                        encryption_key: key,
                        deliver_to,
                        headers,
                    }),
                    timeout,
                },
            )
            .await;

        let success = match response {
            Ok(ResponseVariant::AudioRequestSuccess(s)) => s,
            Ok(ResponseVariant::AudioRequestFailure(f)) => {
                self.record_use(&req, false, false).await;
                return Err(TapHubError::TapScript {
                    reason: f.reason,
                    try_others: f.try_others,
                });
            }
            Ok(_) => {
                self.record_use(&req, false, false).await;
                return Err(TapHubError::Internal(
                    "tap answered an audio request with metadata".to_string(),
                ));
            }
            Err(e) => {
                self.record_use(&req, false, false).await;
                return Err(dispatch_error(e));
            }
        };

        let policy = convert_policy(success.cache);
        let metadatas = self
            .resolve_metadata(&req.tap_id, success.metadatas, &req.audio_request)
            .await;

        // When audio is cached under a CacheKey, also record its metadata under
        // the ARHash. Without it, metadata-only lookups miss forever and go to
        // the tap every time — nothing visibly breaks, which is exactly why it
        // is easy to drop in a rewrite.
        if let Some(item) = &cache_item
            && matches!(item.key, AudioCacheItemKey::CacheKey(_))
        {
            self.write_arhash_alias(item, &req.audio_request, &metadatas, &policy)
                .await;
        }

        self.record_use(&req, false, true).await;

        Ok(AudioDispatch::Dispatched {
            meta: AudioMetaResponse {
                metadatas,
                cache_key: policy,
                base_volume: tap.base_volume,
            },
            cache_item,
        })
    }

    // --- metadata ----------------------------------------------------------

    pub async fn request_audio_meta(
        &self,
        req: AudioRequest,
    ) -> Result<MetaDispatch, TapHubError> {
        let tap = self.resolve_tap(&req.tap_id).await?;
        self.verify_permission(&tap, &req.discord_user_id).await?;

        let meta_key = arhash_key(&req.request);
        if let Some(entry) = self.cache.get_entry(&req.tap_id, &meta_key).await {
            return Ok(MetaDispatch::Ready(AudioMetaResponse {
                metadatas: entry.metadatas,
                cache_key: entry.cache_key,
                base_volume: tap.base_volume,
            }));
        }

        if !tap.gateway_v4 {
            return Ok(MetaDispatch::Legacy);
        }

        let response = self
            .dispatcher
            .dispatch(
                &req.tap_id,
                PendingRequest {
                    request_id: zakofish4_common::model::RequestId(Uuid::new_v4()),
                    variant: RequestVariant::AudioMetadataRequest(AudioMetadataRequestMessage {
                        ars: zakofish4_common::model::AudioRequestString(req.request.to_string()),
                        discord_user_id: zakofish4_common::model::DiscordUserId(
                            req.discord_user_id.0.clone(),
                        ),
                        headers: req.headers.clone(),
                    }),
                    timeout: self.timeouts.metadata,
                },
            )
            .await;

        let meta = match response {
            Ok(ResponseVariant::AudioMetadataSuccess(m)) => m,
            // A refusal the tap authored is the user's answer — "age
            // restricted" is worth showing verbatim, and collapsing it into a
            // generic error is a real loss of information.
            Ok(ResponseVariant::AudioMetadataFailure(f)) => {
                return Err(TapHubError::TapScript {
                    reason: f.reason,
                    try_others: f.try_others,
                });
            }
            Ok(_) => {
                return Err(TapHubError::Internal(
                    "tap answered a metadata request with audio".to_string(),
                ));
            }
            // A transport failure is different: the tap said nothing, so a
            // concurrent writer may have filled the cache in the meantime.
            Err(e) => {
                tracing::warn!(%e, tap_id = %req.tap_id.0, "metadata dispatch failed; retrying cache");
                if let Some(entry) = self.cache.get_entry(&req.tap_id, &meta_key).await {
                    return Ok(MetaDispatch::Ready(AudioMetaResponse {
                        metadatas: entry.metadatas,
                        cache_key: entry.cache_key,
                        base_volume: tap.base_volume,
                    }));
                }
                return Err(TapHubError::TapUnavailable);
            }
        };

        let metadatas: Vec<AudioMetadata> =
            meta.metadatas.into_iter().map(convert_metadata).collect();
        let policy = convert_policy(meta.cache);
        self.store_metadata_async(&req.tap_id, meta_key, &metadatas, &policy);

        Ok(MetaDispatch::Ready(AudioMetaResponse {
            metadatas,
            cache_key: policy,
            base_volume: tap.base_volume,
        }))
    }

    // --- preload -----------------------------------------------------------

    /// Fill the cache without playing anything.
    ///
    /// The one path with no audio engine in it at all: HQ obtains a ticket from
    /// the cache worker, which arms its own receiver, and the tap streams
    /// straight to disk. The cache worker is a protofish4 sink like any other,
    /// which is what lets the whole UDP path be reused here unchanged.
    pub async fn preload_audio(
        &self,
        req: CachedAudioRequest,
    ) -> Result<MetaDispatch, TapHubError> {
        let tap = self.resolve_tap(&req.tap_id).await?;
        self.verify_permission(&tap, &req.discord_user_id).await?;

        let cache_item = build_cache_item(&req.tap_id, &req.cache_key, &req.audio_request);
        if let Some(item) = &cache_item
            && let Some(entry) = self.cache.get_entry(&item.tap_id, &item.key).await
            && entry.has_audio()
        {
            return Ok(MetaDispatch::Ready(AudioMetaResponse {
                metadatas: entry.metadatas,
                cache_key: entry.cache_key,
                base_volume: tap.base_volume,
            }));
        }

        // Everything above is shared with the taphub path; only past here does
        // the protocol diverge, so a tap that has not been migrated must stop
        // at this line.
        if !tap.gateway_v4 {
            return Ok(MetaDispatch::Legacy);
        }

        let Some(ingest) = self.ingest.clone() else {
            return Ok(MetaDispatch::Legacy);
        };
        // Nothing to preload into: without a cache item there is no target for
        // the audio, so this is a metadata request wearing the wrong name.
        let Some(item) = cache_item else {
            return Ok(MetaDispatch::Legacy);
        };

        // A cache worker that cannot take an ingest — not yet deployed with a
        // UDP port, or out of slots — must not fail the preload. Preloading is
        // an optimisation, and the legacy path still fills the same cache.
        let opened = match ingest
            .open(CreateIngestReq {
                item: item.clone(),
                metadatas: Vec::new(),
                cache_key: req.cache_key.clone(),
            })
            .await
        {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!(%e, tap_id = %req.tap_id.0, "cache worker refused an ingest slot");
                return Ok(MetaDispatch::Legacy);
            }
        };

        // Armed from here on. Every early return below goes through the guard.
        let mut guard = IngestGuard {
            ingest: Arc::clone(&ingest),
            preload_id: Some(opened.preload_id),
            request_id: Some(opened.request_id),
            sinks: self.sinks.clone(),
        };

        let ticket = SinkTicket {
            request_id: opened.request_id,
            encryption_key: opened.encryption_key,
        };
        if !ticket.looks_valid() {
            return Err(TapHubError::Internal(
                "cache worker returned a malformed sink ticket".to_string(),
            ));
        }
        let deliver_to = self
            .resolve_deliver_to(&self.cache_sink_id, &ticket)
            .await?;

        let response = self
            .dispatcher
            .dispatch(
                &req.tap_id,
                PendingRequest {
                    request_id: zakofish4_common::model::RequestId(ticket.request_id),
                    variant: RequestVariant::AudioRequest(AudioRequestMessage {
                        ars: zakofish4_common::model::AudioRequestString(
                            req.audio_request.to_string(),
                        ),
                        discord_user_id: zakofish4_common::model::DiscordUserId(
                            req.discord_user_id.0.clone(),
                        ),
                        encryption_key: EncryptionKey(ticket.encryption_key),
                        deliver_to,
                        headers: req.headers.clone(),
                    }),
                    timeout: self.timeouts.preload,
                },
            )
            .await;

        let success = match response {
            Ok(ResponseVariant::AudioRequestSuccess(s)) => s,
            Ok(ResponseVariant::AudioRequestFailure(f)) => {
                return Err(TapHubError::TapScript {
                    reason: f.reason,
                    try_others: f.try_others,
                });
            }
            Ok(_) => {
                return Err(TapHubError::Internal(
                    "tap answered a preload with metadata".to_string(),
                ));
            }
            Err(e) => return Err(dispatch_error(e)),
        };

        let policy = convert_policy(success.cache);
        let metadatas = self
            .resolve_metadata(&req.tap_id, success.metadatas, &req.audio_request)
            .await;

        // Releases the session to commit once the last frame lands. The tap is
        // already streaming, so this is a race the cache worker resolves: it
        // commits from whichever of the two halves finishes second.
        ingest
            .finalize(
                opened.preload_id,
                FinalizeIngestReq {
                    metadatas: metadatas.clone(),
                    cache_key: policy.clone(),
                },
            )
            .await
            .map_err(|e| TapHubError::Internal(format!("failed to finalize an ingest slot: {e}")))?;
        guard.disarm();

        if matches!(item.key, AudioCacheItemKey::CacheKey(_)) {
            self.write_arhash_alias(&item, &req.audio_request, &metadatas, &policy)
                .await;
        }

        Ok(MetaDispatch::Ready(AudioMetaResponse {
            metadatas,
            cache_key: policy,
            base_volume: tap.base_volume,
        }))
    }

    // --- invalidation ------------------------------------------------------

    /// Drop an entry an audio engine could not decode.
    ///
    /// Losing this leaves a poisoned entry being served and failing for as long
    /// as it survives eviction.
    pub async fn invalidate_cache(&self, req: CachedAudioRequest) -> Result<(), TapHubError> {
        let Some(item) = build_cache_item(&req.tap_id, &req.cache_key, &req.audio_request) else {
            return Ok(());
        };

        self.cache
            .delete(&item.tap_id, &item.key)
            .await
            .map_err(|e| TapHubError::Internal(format!("failed to delete cache entry: {e}")))?;

        tracing::warn!(
            tap_id = %req.tap_id.0,
            key = %item.key,
            "cache entry invalidated after a decode failure"
        );
        Ok(())
    }

    // --- stream outcomes ---------------------------------------------------

    /// Record how a transfer ended, and release its proxy route.
    pub async fn report_stream_outcome(&self, request_id: Uuid, report: StreamReport) {
        match &report {
            StreamReport::Completed { frames_sent } => {
                tracing::info!(%request_id, frames_sent, "stream completed");
            }
            StreamReport::Aborted { frames_sent, reason } => {
                tracing::warn!(%request_id, frames_sent, %reason, "stream aborted");
            }
            // The one that indicts the network path rather than the tap, and
            // therefore the one worth alerting on.
            StreamReport::Undeliverable { reason } => {
                tracing::error!(%request_id, %reason, "audio could not be delivered to its sink");
            }
        }
        self.sinks.clear_route(&request_id).await;
    }

    // --- internals ---------------------------------------------------------

    async fn resolve_tap(&self, tap_id: &TapId) -> Result<Tap, TapHubError> {
        self.tap_repo
            .find_by_id(tap_id.clone())
            .await
            .map_err(|e| TapHubError::Internal(e.to_string()))?
            .ok_or_else(|| TapHubError::TapNotFound(tap_id.0.clone()))
    }

    /// Mirrors the RPC's `verify_tap_permission`, minus the round trip.
    ///
    /// Resolving the Discord user is best-effort: `check_access` treats an
    /// unknown user as anonymous, which is the right answer for a tap that is
    /// public and the right refusal for one that is not.
    async fn verify_permission(
        &self,
        tap: &Tap,
        user: &DiscordUserId,
    ) -> Result<(), TapHubError> {
        let user_id = match self.tap.get_user_by_discord_id(&user.0).await {
            Ok(Some(u)) => std::str::FromStr::from_str(&u.id.0).ok(),
            _ => None,
        };
        if self.tap.check_access(tap, user_id).await {
            Ok(())
        } else {
            Err(TapHubError::PermissionDenied(tap.id.0.clone()))
        }
    }

    /// A usable cached copy, or `None`.
    ///
    /// "Usable" is stricter than "present": an entry still being written has a
    /// row but only part of a file, and serving it would hand the listener
    /// truncated audio. The reader check catches the other direction, where the
    /// row outlived the file.
    async fn try_cache_hit(
        &self,
        tap: &Tap,
        item: Option<&AudioCacheItem>,
    ) -> Option<(AudioMetaResponse, AudioCacheItem)> {
        let item = item?;
        let entry = self.cache.get_entry(&item.tap_id, &item.key).await?;
        if !entry.has_audio() || entry.is_downloading() {
            return None;
        }
        if self.cache.open_reader(&item.tap_id, &item.key).await.is_none() {
            tracing::warn!(
                tap_id = %item.tap_id.0,
                key = %item.key,
                "cache entry exists but its file could not be opened"
            );
            return None;
        }
        Some((
            AudioMetaResponse {
                metadatas: entry.metadatas,
                cache_key: entry.cache_key,
                base_volume: tap.base_volume,
            },
            item.clone(),
        ))
    }

    /// Where the tap should send, and the proxy route to get it there.
    async fn resolve_deliver_to(
        &self,
        sink_id: &str,
        ticket: &SinkTicket,
    ) -> Result<DeliverTo, TapHubError> {
        let Some(ad) = self.sinks.get(sink_id).await else {
            return Err(TapHubError::Internal(format!(
                "sink {sink_id} has not advertised an address"
            )));
        };

        // Strict ordering: the route must exist before the tap is told to send,
        // or the proxy drops the opening datagrams for a request it has never
        // heard of. Local to this function, which is the whole reason
        // receiver-minting beats a distributed handshake.
        self.sinks
            .publish_route(&ticket.request_id, &ad.internal_addr)
            .await
            .map_err(|e| TapHubError::Internal(format!("failed to publish proxy route: {e}")))?;

        let deliver_to = self
            .sinks
            .deliver_to(sink_id, self.proxy_addr.as_deref())
            .await;
        if deliver_to.is_empty() {
            return Err(TapHubError::Internal(format!(
                "no reachable address for sink {sink_id}"
            )));
        }
        Ok(deliver_to)
    }

    /// Turn a tap's `UseCached` into real metadata.
    ///
    /// The variant survives a rewrite trivially; the substitution behind it does
    /// not, and on a miss the result is empty metadata rather than an error —
    /// so forgetting this shows up as tracks with no title, not as a failure.
    async fn resolve_metadata(
        &self,
        tap_id: &TapId,
        attached: AttachedMetadata,
        ars: &AudioRequestString,
    ) -> Vec<AudioMetadata> {
        match attached {
            AttachedMetadata::Metadatas(v) => v
                .into_iter()
                .map(convert_metadata)
                .collect(),
            AttachedMetadata::UseCached => {
                match self.cache.get_entry(tap_id, &arhash_key(ars)).await {
                    Some(entry) => entry.metadatas,
                    None => {
                        tracing::warn!(
                            tap_id = %tap_id.0,
                            "tap asked to reuse cached metadata, but none is cached"
                        );
                        Vec::new()
                    }
                }
            }
        }
    }

    async fn write_arhash_alias(
        &self,
        item: &AudioCacheItem,
        ars: &AudioRequestString,
        metadatas: &[AudioMetadata],
        policy: &AudioCachePolicy,
    ) {
        let alias = AudioCacheItem {
            key: arhash_key(ars),
            tap_id: item.tap_id.clone(),
            expire_at: item.expire_at,
        };
        if let Err(e) = self
            .cache
            .store_metadata(alias, metadatas.to_vec(), policy.clone())
            .await
        {
            tracing::warn!(%e, "failed to write the ARHash metadata alias");
        }
    }

    fn store_metadata_async(
        &self,
        tap_id: &TapId,
        key: AudioCacheItemKey,
        metadatas: &[AudioMetadata],
        policy: &AudioCachePolicy,
    ) {
        let item = AudioCacheItem {
            key,
            tap_id: tap_id.clone(),
            expire_at: policy
                .ttl_seconds
                .map(|ttl| Utc::now() + chrono::Duration::seconds(ttl as i64)),
        };
        let cache = Arc::clone(&self.cache);
        let metadatas = metadatas.to_vec();
        let policy = policy.clone();
        tokio::spawn(async move {
            if let Err(e) = cache.store_metadata(item, metadatas, policy).await {
                tracing::warn!(%e, "failed to store metadata in the cache");
            }
        });
    }

    /// Record a request against the tap's usage numbers and the history feed.
    ///
    /// `success` is a parameter rather than a constant. taphub hardcoded it to
    /// `true` in both call sites, so every failed play has been recorded as a
    /// success — these are the taps' billing figures, so that is worth fixing
    /// rather than porting.
    async fn record_use(&self, req: &CachedAudioRequest, cache_hit: bool, success: bool) {
        let tap_id = req.tap_id.clone();
        let entry = UseHistoryEntry::PlayAudio(PlayAudioHistory {
            user_id: None,
            discord_user_id: Some(req.discord_user_id.clone()),
            ars_length: req.audio_request.to_string().len(),
            trace_id: current_trace_id(),
            tap_id: tap_id.clone(),
            cache_hit,
            success,
        });

        let metrics = self.metrics.clone();
        let history = Arc::clone(&self.history);
        tokio::spawn(async move {
            let _ = metrics.incr_delta_total_uses(&tap_id).await;
            if cache_hit {
                let _ = metrics.incr_delta_cache_hits(&tap_id).await;
            }
            if let Err(e) = history.publish_history(&entry).await {
                tracing::warn!(%e, "failed to publish use history");
            }
        });
    }
}

/// Build the cache target for a request, or `None` when it is not cacheable.
pub fn build_cache_item(
    tap_id: &TapId,
    policy: &AudioCachePolicy,
    ars: &AudioRequestString,
) -> Option<AudioCacheItem> {
    let expire_at = policy
        .ttl_seconds
        .map(|ttl| Utc::now() + chrono::Duration::seconds(ttl as i64));

    let key = match &policy.cache_type {
        AudioCacheType::None => return None,
        AudioCacheType::ARHash => arhash_key(ars),
        AudioCacheType::CacheKey(k) => AudioCacheItemKey::CacheKey(k.clone()),
    };

    Some(AudioCacheItem {
        key,
        tap_id: tap_id.clone(),
        expire_at,
    })
}

/// The cache key a request hashes to. Metadata is filed under this regardless
/// of where the audio itself lives.
pub fn arhash_key(ars: &AudioRequestString) -> AudioCacheItemKey {
    AudioCacheItemKey::ARHash(hex::encode(sha2::Sha256::digest(
        ars.to_string().as_bytes(),
    )))
}

fn dispatch_error(e: DispatchFailure) -> TapHubError {
    match e {
        DispatchFailure::NoConnection => TapHubError::TapUnavailable,
        DispatchFailure::Timeout => {
            TapHubError::Internal("tap request timed out".to_string())
        }
        DispatchFailure::Disconnected => TapHubError::TapUnavailable,
        DispatchFailure::Transport(m) => TapHubError::Internal(m),
    }
}

/// Bridge the gateway's cache policy to HQ's.
///
/// The two are structurally identical but live in different crates: the wire
/// type belongs to the protocol and HQ's belongs to the domain, and letting one
/// be the other would make every future protocol change a database change.
pub fn convert_policy(p: zakofish4_common::model::AudioCachePolicy) -> AudioCachePolicy {
    use zakofish4_common::model::AudioCacheType as W;
    AudioCachePolicy {
        cache_type: match p.cache_type {
            W::None => AudioCacheType::None,
            W::ARHash => AudioCacheType::ARHash,
            W::CacheKey(k) => AudioCacheType::CacheKey(k),
        },
        ttl_seconds: p.ttl_seconds,
    }
}

/// Bridge the gateway's metadata type to HQ's. Same shape, two crates.
pub fn convert_metadata(m: zakofish4_common::model::AudioMetadata) -> AudioMetadata {
    use zakofish4_common::model::AudioMetadata as W;
    match m {
        W::Title(v) => AudioMetadata::Title(v),
        W::Description(v) => AudioMetadata::Description(v),
        W::Artist(v) => AudioMetadata::Artist(v),
        W::Album(v) => AudioMetadata::Album(v),
        W::ImageUrl(v) => AudioMetadata::ImageUrl(v),
        W::Url(v) => AudioMetadata::Url(v),
    }
}

/// The active trace, for correlating a request across services.
///
/// The UDP leg carries no trace context at all, so `request_id` is the only
/// thing tying the two halves together — which is why it is worth recording
/// here alongside the trace.
fn current_trace_id() -> Option<String> {
    use opentelemetry::trace::TraceContextExt;
    use tracing_opentelemetry::OpenTelemetrySpanExt;
    let id = tracing::Span::current()
        .context()
        .span()
        .span_context()
        .trace_id()
        .to_string();
    if id == "00000000000000000000000000000000" {
        None
    } else {
        Some(id)
    }
}
