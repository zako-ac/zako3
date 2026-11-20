mod jitter;

use protofish3::xfer::XferRecv;
use protofish3::{
    ChanReceiver, ChanSender, Client, ClientConfig, ReconnectConfig, ReconnectingClient, XferMode,
};
use rustls::pki_types::CertificateDer;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use zako3_audio_probe::{
    BufferView, RequestView, StallInputs, StreamProbe, StreamTrace, UpstreamHopSource,
    build_report, current_trace, emit, now_unix_ms, request_stream_id,
};
use zako3_opus_jitter::{BufferStats, JitterConfig, JitterError, OpusJitterBuffer};
use zako3_taphub_transport_lib::{TapHubRequest, TapHubResponse};
use zako3_types::{
    AudioCacheType, AudioMetaResponse, AudioRequest, AudioResponse, CachedAudioRequest, TapHubError,
};

const RESET_CLOSE_CODE: u64 = 1;
const MAX_ATTEMPTS: usize = 2;
/// Number of independent QUIC connections (all over one shared client endpoint) that
/// requests are round-robined across. A transport-level reset is scoped to the single
/// connection a failing request landed on, so it never tears down siblings on the other
/// pooled connections — the whole point of the pool.
const POOL_SIZE: usize = 4;
/// How long an abandoned track waits for the peer to end the transfer on its own
/// before the receive side is ended by hand.
///
/// Sending `RecvEnd` while the peer's own end-of-transfer frame is already in flight
/// is a protocol violation, and the driver treats one as fatal for the whole
/// connection — the very thing this abort path exists to avoid. Waiting one round
/// trip lets the common case (a listener that left while the tap was finishing)
/// resolve itself with nothing on the wire.
const XFER_DRAIN_GRACE: Duration = Duration::from_millis(500);
/// Bound on the handshake that ends the receive side.
const XFER_END_TIMEOUT: Duration = Duration::from_secs(1);
/// How many exchanges in a row may time out on one pooled connection before
/// `pick_conn` stops handing it requests.
///
/// Without a reset there is nothing else that evicts a *wedged* connection — one
/// whose peer still answers QUIC keepalives but never answers an application
/// request. Left in the round-robin it would make one exchange in `POOL_SIZE` pay
/// the full timeout for the life of the process.
const WEDGED_AFTER: u32 = 2;
/// How long a wedged connection is kept out of the rotation before one request is
/// allowed to probe it again.
///
/// The quarantine has to expire: while a connection is skipped nothing is sent to
/// it, so no success can ever clear the count, and a permanent skip would silently
/// shrink the pool for the rest of the process after one transient slowdown. After
/// this window the next pick lands on it, and an answer puts it straight back into
/// full rotation.
const WEDGE_COOLDOWN: Duration = Duration::from_secs(30);
/// Sentinel for "this client has never completed an exchange" in `last_success_ms`.
const NEVER: u64 = u64::MAX;

/// What a supervisor needs in order to decide whether this client is still usable.
///
/// The counters describe *exchanges*, not connections: a request that fails on
/// both attempts is one failure, and any successful answer clears the count.
/// A track that stalls mid-stream is not counted — its exchange succeeded and
/// the silence is downstream of the request path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientHealth {
    /// Requests that failed on every attempt since the last successful exchange.
    pub consecutive_failures: u64,
    /// Time since the last successful exchange; `None` if there has not been one.
    pub since_last_success: Option<Duration>,
    /// Reset attempts: transport failures plus `close()`.
    pub resets: u64,
}

impl ClientHealth {
    /// True once `min_failures` requests in a row have failed outright.
    ///
    /// One failure is not enough evidence to throw the client away: a taphub
    /// restart or a brief network blip fails one or two requests and then
    /// heals on its own. At production budgets (2 attempts of 6s) three in a
    /// row means roughly half a minute of uninterrupted failure.
    pub fn is_wedged(&self, min_failures: u64) -> bool {
        self.consecutive_failures >= min_failures
    }
}

pub struct TransportClient {
    /// Pool of independent reconnecting connections. Requests round-robin across them via
    /// `cursor` so no single connection is a shared point of failure for the whole fleet.
    conns: Vec<Arc<ReconnectingClient>>,
    cursor: AtomicUsize,
    request_timeout: Duration,
    /// Where to read taphub's side of a stalled stream's hop record. `None`
    /// leaves the two upstream hops out of a stall report, which the report
    /// then says rather than guessing at.
    upstream_hops: Option<Arc<dyn UpstreamHopSource>>,
    /// Consecutive timeouts per pooled connection, parallel to `conns`.
    timeouts: Vec<AtomicU32>,
    /// When a wedged connection may be tried again, in milliseconds since `started`.
    /// Zero means "not wedged"; see `WEDGE_COOLDOWN`.
    wedged_until_ms: Vec<AtomicU64>,
    /// Base for the cooldown clock, so a deadline is a plain atomic integer.
    started: Instant,
    /// Per-track jitter buffer settings. Production runs the defaults; tests shorten
    /// `stall_timeout` so a stall can be observed in a second instead of fifteen.
    jitter: JitterConfig,
    /// Pooled connections this client has reset since it was built.
    ///
    /// Counts reset attempts: a reset tears down every stream multiplexed over that
    /// connection, so it is a last resort kept for a transport that is really broken —
    /// a request that merely timed out must not move this counter.
    resets: AtomicU64,
    /// Requests that failed on every attempt since the last successful exchange.
    ///
    /// Kept at the client rather than per connection because the caller's
    /// recovery is client-wide: when every pooled connection has stopped
    /// answering, the honest replacement is a new client (new endpoint, new
    /// connections), not another round over the same dead pool.
    consecutive_failures: AtomicU64,
    /// When the last exchange succeeded, in milliseconds since `started`;
    /// `NEVER` until the first one.
    last_success_ms: AtomicU64,
}

impl TransportClient {
    pub async fn connect(
        bind_addr: SocketAddr,
        server_addr: &str,
        server_name: String,
        root_certificates: Vec<CertificateDer<'static>>,
        request_timeout: Duration,
    ) -> Result<Self, String> {
        let server_addr = server_addr
            .to_socket_addrs()
            .map_err(|e| format!("Failed to resolve '{}': {}", server_addr, e))?
            .next()
            .ok_or_else(|| format!("No addresses resolved for '{}'", server_addr))?;

        let mut config = ClientConfig::new(bind_addr);
        config.root_certificates = root_certificates;
        config.handshake_timeout = Duration::from_secs(10);
        config.protofish.xfer_credit_update_batch_size = Some(10);
        // Tight keepalive so stale (black-holed) connections are detected within
        // seconds instead of quinn's 45s default idle window.
        config.protofish.keepalive_interval = Some(Duration::from_secs(5));
        config.protofish.keepalive_timeout = Some(Duration::from_secs(15));

        // One shared QUIC client endpoint underlies every pooled connection; QUIC
        // multiplexes the independent connections over it.
        let client = Arc::new(Client::bind(config).map_err(|e| e.to_string())?);

        let reconnect_config = ReconnectConfig {
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            max_retries: None,
            pool_size: 1,
        };

        let mut conns = Vec::with_capacity(POOL_SIZE);
        for _ in 0..POOL_SIZE {
            let conn = ReconnectingClient::connect(
                Arc::clone(&client),
                server_addr,
                server_name.clone(),
                HashMap::new(),
                reconnect_config.clone(),
            )
            .await
            .map_err(|e| e.to_string())?;
            conns.push(Arc::new(conn));
        }

        Ok(Self {
            conns,
            cursor: AtomicUsize::new(0),
            request_timeout,
            upstream_hops: None,
            timeouts: (0..POOL_SIZE).map(|_| AtomicU32::new(0)).collect(),
            wedged_until_ms: (0..POOL_SIZE).map(|_| AtomicU64::new(0)).collect(),
            started: Instant::now(),
            jitter: JitterConfig::default(),
            resets: AtomicU64::new(0),
            consecutive_failures: AtomicU64::new(0),
            last_success_ms: AtomicU64::new(NEVER),
        })
    }

    /// Closes every pooled connection, code `RESET_CLOSE_CODE`.
    ///
    /// The server observes this as the client going away, so any request still
    /// in flight for this client is cancelled there instead of being served into
    /// a connection nobody reads. The tests use it for exactly that; a shutdown
    /// path could use it too. It is not what a slow request does — that one
    /// abandons its own channel and leaves the connection up.
    pub async fn close(&self) {
        for conn in &self.conns {
            self.resets.fetch_add(1, Ordering::Relaxed);
            conn.reset(RESET_CLOSE_CODE).await;
        }
    }

    /// Replace the jitter buffer settings used for every track this client streams.
    ///
    /// Tuning and tests only; production takes the defaults. A shorter
    /// `stall_timeout` is how a stall is reproduced in a test without waiting the
    /// fifteen seconds the audit measured.
    pub fn with_jitter_config(mut self, jitter: JitterConfig) -> Self {
        self.jitter = jitter;
        self
    }

    /// Attach the reader for taphub's hop record.
    ///
    /// Set once, at construction; a stall report is the only reader.
    pub fn with_upstream_hops(mut self, source: Arc<dyn UpstreamHopSource>) -> Self {
        self.upstream_hops = Some(source);
        self
    }

    /// Reset attempts this client has made, on `close()` or on a transport failure.
    ///
    /// Stays at zero while requests only time out or tracks only stall: neither
    /// resets a connection now.
    pub fn resets_total(&self) -> u64 {
        self.resets.load(Ordering::Relaxed)
    }

    /// This client's exchange health, for a supervisor deciding whether to
    /// replace it. Cheap: three atomic loads.
    pub fn health(&self) -> ClientHealth {
        let last = self.last_success_ms.load(Ordering::Relaxed);
        ClientHealth {
            consecutive_failures: self.consecutive_failures.load(Ordering::Relaxed),
            since_last_success: (last != NEVER)
                .then(|| Duration::from_millis(self.now_ms().saturating_sub(last))),
            resets: self.resets.load(Ordering::Relaxed),
        }
    }

    /// An exchange got an answer: whatever was failing is over.
    fn record_success(&self) {
        self.last_success_ms.store(self.now_ms(), Ordering::Relaxed);
        self.consecutive_failures.store(0, Ordering::Relaxed);
    }

    /// An exchange failed on every attempt.
    fn record_failure(&self) {
        self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Round-robin one connection out of the pool for a single attempt, skipping
    /// connections that have timed out `WEDGED_AFTER` times in a row.
    ///
    /// When every connection is wedged the rotation is used anyway: refusing to
    /// send would turn a taphub-wide slowdown into an outage of our own.
    fn pick_conn(&self) -> (usize, Arc<ReconnectingClient>) {
        let n = self.conns.len();
        let start = self.cursor.fetch_add(1, Ordering::Relaxed) % n;
        let now = self.now_ms();
        let idx = pick_index(start, n, |i| {
            self.timeouts[i].load(Ordering::Relaxed) >= WEDGED_AFTER
                && now < self.wedged_until_ms[i].load(Ordering::Relaxed)
        });
        (idx, Arc::clone(&self.conns[idx]))
    }

    /// Milliseconds since this client was built, for the cooldown deadline.
    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    /// Record a timeout on `idx`; `WEDGED_AFTER` in a row starts a cooldown.
    fn note_timeout(&self, idx: usize) {
        let count = self.timeouts[idx].fetch_add(1, Ordering::Relaxed) + 1;
        if count >= WEDGED_AFTER {
            let until = self.now_ms() + WEDGE_COOLDOWN.as_millis() as u64;
            self.wedged_until_ms[idx].store(until, Ordering::Relaxed);
        }
    }

    /// Record an answer on `idx`: it is healthy and back in full rotation.
    fn note_ok(&self, idx: usize) {
        self.timeouts[idx].store(0, Ordering::Relaxed);
        self.wedged_until_ms[idx].store(0, Ordering::Relaxed);
    }

    async fn round_trip_once(
        conn: &ReconnectingClient,
        payload: &[u8],
    ) -> protofish3::Result<(ChanSender, ChanReceiver, Vec<u8>)> {
        let (sender, mut receiver) = conn.open_chan().await?;
        sender.send_msg(payload.to_vec()).await?;
        let resp_payload = receiver.recv_msg().await?;
        Ok((sender, receiver, resp_payload))
    }

    /// One request/response exchange with a per-attempt timeout, isolated to a single
    /// pooled connection so a failure here never disturbs sibling requests on the other
    /// pooled connections. Recovery is scoped:
    /// - **Timeout**: only this request's channel is abandoned, by dropping it. The
    ///   connection stays up — every track streaming over it would die with a reset —
    ///   and the retry moves to the next connection in the pool instead. A connection
    ///   that is genuinely dead is already evicted by the transport keepalive (5s
    ///   interval, 15s timeout); one that stays alive but never answers is counted and
    ///   skipped by `pick_conn` for `WEDGE_COOLDOWN` after `WEDGED_AFTER` timeouts, and
    ///   probed again when that window closes.
    /// - **Transport error**: reset only *this* pooled connection, then retry.
    ///
    /// Application-level `TapHubResponse::Error` is returned as-is, never retried.
    ///
    /// Returns the pooled connection the exchange succeeded on so callers that keep
    /// streaming (e.g. `request_audio`) stay on the same connection.
    async fn round_trip(
        &self,
        payload: Vec<u8>,
    ) -> Result<
        (
            Arc<ReconnectingClient>,
            ChanSender,
            ChanReceiver,
            TapHubResponse,
        ),
        TapHubError,
    > {
        for attempt in 1..=MAX_ATTEMPTS {
            // Picked per attempt, not once per exchange: a retry after a timeout must
            // not go back onto the connection that just failed to answer.
            let (idx, conn) = self.pick_conn();
            match tokio::time::timeout(self.request_timeout, Self::round_trip_once(&conn, &payload))
                .await
            {
                Ok(Ok((sender, receiver, resp_payload))) => {
                    let resp: TapHubResponse = rmp_serde::from_slice(&resp_payload)
                        .map_err(|e| TapHubError::Internal(e.to_string()))?;
                    // It answered, so whatever made it slow before is gone.
                    self.note_ok(idx);
                    self.record_success();
                    return Ok((conn, sender, receiver, resp));
                }
                Ok(Err(e)) if attempt < MAX_ATTEMPTS && is_transport_error(&e) => {
                    tracing::warn!(
                        error = %e,
                        attempt,
                        "taphub request failed on transport; resetting this pooled connection and retrying"
                    );
                    // Scoped to this one pooled connection — siblings are untouched.
                    self.resets.fetch_add(1, Ordering::Relaxed);
                    conn.reset(RESET_CLOSE_CODE).await;
                    // A fresh connection has no history of timing out.
                    self.note_ok(idx);
                }
                Ok(Err(e)) => {
                    self.record_failure();
                    return Err(TapHubError::Internal(e.to_string()));
                }
                Err(_) if attempt < MAX_ATTEMPTS => {
                    tracing::warn!(
                        timeout_ms = self.request_timeout.as_millis() as u64,
                        attempt,
                        "taphub request timed out; abandoning only this request's channel and retrying on another pooled connection"
                    );
                    // Deliberately no `conn.reset` here: dropping the timed-out future
                    // closes this request's channel, while a reset would cut off every
                    // other track multiplexed over the same connection. Blame the
                    // connection instead, so a wedged one leaves the rotation.
                    self.note_timeout(idx);
                }
                Err(_) => {
                    self.note_timeout(idx);
                    self.record_failure();
                    return Err(TapHubError::Internal(format!(
                        "taphub request timed out after {} attempts ({}ms each)",
                        MAX_ATTEMPTS,
                        self.request_timeout.as_millis()
                    )));
                }
            }
        }
        unreachable!("round_trip loop always returns")
    }

    async fn execute_request(&self, req: TapHubRequest) -> Result<TapHubResponse, TapHubError> {
        let payload = rmp_serde::to_vec(&req).map_err(|e| TapHubError::Internal(e.to_string()))?;
        let (_conn, _sender, _receiver, resp) = self.round_trip(payload).await?;
        Ok(resp)
    }

    /// Request a stream, taking the track identity from the task-local trace the
    /// session layer scoped around this call (see `zako3_audio_probe::with_trace`).
    pub async fn request_audio(
        &self,
        req: CachedAudioRequest,
    ) -> Result<AudioResponse, TapHubError> {
        self.request_audio_tagged(req, current_trace()).await
    }

    /// `request_audio` with an explicit trace, for a caller that has one without
    /// a task-local.
    pub async fn request_audio_tagged(
        &self,
        req: CachedAudioRequest,
        trace: Option<StreamTrace>,
    ) -> Result<AudioResponse, TapHubError> {
        let req_clone = req.clone();
        // The stream id both ends derive from the request, so taphub's hop
        // record for this stream can be found by key.
        let stream = request_stream_id(
            &req.tap_id.0,
            &req.discord_user_id.0.to_string(),
            &req.audio_request.to_string(),
        );
        let request_view = RequestView {
            tap_id: req.tap_id.0.to_string(),
            cache_policy: Some(match &req.cache_key.cache_type {
                AudioCacheType::None => "none".to_string(),
                AudioCacheType::ARHash => "ar_hash".to_string(),
                AudioCacheType::CacheKey(k) => format!("key:{k}"),
            }),
            ars_hash: stream
                .rsplit_once('-')
                .map(|(_, h)| h.to_string())
                .unwrap_or_default(),
            ars_len: req.audio_request.to_string().len(),
        };
        let upstream_hops = self.upstream_hops.clone();
        let payload = rmp_serde::to_vec(&TapHubRequest::RequestAudio(req))
            .map_err(|e| TapHubError::Internal(e.to_string()))?;
        let (conn, sender, mut receiver, resp) = self.round_trip(payload).await?;

        match resp {
            TapHubResponse::AudioReady(meta) => {
                let (tx, rx) = mpsc::channel(100);
                // Stream/invalidate on the same pooled connection the request landed on.
                let conn_clone = conn;
                let xfer_timeout = self.request_timeout;
                let invalidate_timeout = self.request_timeout;
                let jitter_config = self.jitter.clone();

                tokio::spawn(async move {
                    // Keep the sender alive for the duration of the transfer so the
                    // chan isn't half-closed before the unreliable xfer completes.
                    let _sender = sender;

                    let xfer =
                        match tokio::time::timeout(xfer_timeout, receiver.accept_xfer()).await {
                            Ok(Ok(x)) => x,
                            Ok(Err(e)) => {
                                tracing::error!("accept_xfer failed: {:?}", e);
                                return;
                            }
                            Err(_) => {
                                tracing::error!(
                                    timeout_ms = xfer_timeout.as_millis() as u64,
                                    "accept_xfer timed out"
                                );
                                return;
                            }
                        };

                    let mut single = match xfer {
                        XferRecv::Single(s) if s.mode() == XferMode::Unrel => s,
                        _ => {
                            tracing::error!("Expected Unrel single transfer");
                            return;
                        }
                    };

                    let probe = Arc::new(StreamProbe::new(stream.clone()));
                    let (frame_tx, frame_rx) = mpsc::channel(jitter::CHANNEL_CAP);
                    let stall_timeout = jitter_config.stall_timeout;
                    let mut jitter = match OpusJitterBuffer::new(frame_rx, jitter_config) {
                        Ok(j) => j,
                        Err(e) => {
                            tracing::error!("Failed to create jitter buffer: {:?}", e);
                            return;
                        }
                    };

                    // The stall reporter is a task of its own: assembling a
                    // report reads another process's hop record, which must
                    // never sit in the playback path.
                    let (stall_tx, mut stall_rx) = mpsc::unbounded_channel();
                    jitter.set_stall_notifier(stall_tx);
                    {
                        let probe = Arc::clone(&probe);
                        let upstream_hops = upstream_hops.clone();
                        let request_view = request_view.clone();
                        let trace = trace.clone();
                        let stream = stream.clone();
                        tokio::spawn(async move {
                            while let Some(event) = stall_rx.recv().await {
                                // Read the probe *before* the round trip to
                                // taphub: the report describes the instant the
                                // buffer gave up, and a probe read afterwards
                                // would describe a later one.
                                let snapshot = probe.snapshot();
                                let now = now_unix_ms();
                                let upstream = match &upstream_hops {
                                    Some(source) => source.read(&stream).await,
                                    None => None,
                                };
                                let report = build_report(StallInputs {
                                    stall_ms: event.stalled_ms,
                                    now_unix_ms: now,
                                    probe: snapshot,
                                    buffer: buffer_view(&event.stats),
                                    trace: trace.clone(),
                                    request: request_view.clone(),
                                    upstream,
                                });
                                emit(&report);
                            }
                        });
                    }

                    // The pump and the consumer race for the life of the track, and the
                    // task ends when either is done with it. `join!` here was the bug:
                    // a consumer that gave up on a stalled source left the pump alive,
                    // the PCM sender was never dropped, the decoder never saw EOF and
                    // the mixer never ended the track. Aborting the pump is what ends
                    // it — the transfer borrows the channel receiver, so the pump cannot
                    // be a task of its own.
                    let end = {
                        let pump = jitter::pump(&mut single, frame_tx, Some(Arc::clone(&probe)));
                        tokio::pin!(pump);

                        let consume = async {
                            loop {
                                match jitter.yield_pcm().await {
                                    Ok(Some(pcm)) => {
                                        if tx.send(pcm).await.is_err() {
                                            return TrackEnd::ConsumerGone;
                                        }
                                    }
                                    Ok(None) => return TrackEnd::Eof, // Stream ended
                                    Err(e) => {
                                        if matches!(e, JitterError::Stalled(_)) {
                                            // The source went quiet for longer than the
                                            // buffer waits: a wedge upstream, not the end
                                            // of the stream. This is the case that used to
                                            // leave the track hanging forever.
                                            tracing::warn!(
                                                stall_ms = stall_timeout.as_millis() as u64,
                                                "jitter buffer stalled; ending the track"
                                            );
                                            return TrackEnd::Stalled;
                                        }
                                        tracing::error!("Jitter buffer error: {:?}", e);
                                        // Defensive: no `JitterError` variant reaches
                                        // this branch today (the buffer reports only
                                        // `Opus` and `Stalled`), so a corrupt packet
                                        // currently ends as `decode-failure`. Kept so
                                        // the cache-invalidation path is already here
                                        // if the buffer grows that variant.
                                        if e.to_string().contains("InvalidPacket") {
                                            tracing::warn!(
                                                "Invalid opus packet detected; invalidating cache"
                                            );
                                            send_invalidate_cache(
                                                conn_clone,
                                                req_clone,
                                                invalidate_timeout,
                                            )
                                            .await;
                                            return TrackEnd::InvalidPacket;
                                        }
                                        return TrackEnd::DecodeFailure;
                                    }
                                }
                            }
                        };
                        tokio::pin!(consume);

                        tokio::select! {
                            end = consume.as_mut() => end,
                            // The transfer finished before playback did. That is not a
                            // reason to cut off the audio still buffered: its sender is
                            // gone, so the channel is closed and the consumer drains
                            // what is left and reports the end of the stream itself.
                            _ = pump.as_mut() => consume.as_mut().await,
                        }
                    };
                    // The pinned futures die with that block, which releases the borrow
                    // on the transfer so it can be ended cleanly here.

                    if !matches!(end, TrackEnd::Eof) {
                        // Let the peer end the transfer itself where it can. Frames
                        // that land here are discarded: the consumer is gone.
                        let peer_finished = tokio::time::timeout(XFER_DRAIN_GRACE, async {
                            while single.recv().await.is_some() {}
                        })
                        .await
                        .is_ok();

                        if !peer_finished {
                            // Still open, so say the receive side is over rather than
                            // leave the peer pushing frames into nothing.
                            match tokio::time::timeout(XFER_END_TIMEOUT, single.end()).await {
                                Ok(Ok(())) => {}
                                Ok(Err(e)) => tracing::warn!(
                                    reason = end.reason(),
                                    error = %e,
                                    "peer refused the end of the receive side"
                                ),
                                Err(_) => tracing::warn!(
                                    reason = end.reason(),
                                    "timed out ending the receive side of an abandoned transfer"
                                ),
                            }
                        }
                    }

                    tracing::info!(
                        reason = end.reason(),
                        "audio track finished; the PCM sender is being dropped"
                    );

                    if jitter.dropped_frames() > 0 {
                        // The pf3 path has no back channel to the tap, so the
                        // buffer warns as it drops and this is the total for the
                        // track. A nonzero count means audio was decimated.
                        tracing::warn!(
                            dropped = jitter.dropped_frames(),
                            "frames dropped: the sender ran ahead of playback"
                        );
                    }
                });

                Ok(AudioResponse {
                    cache_key: Some(meta.cache_key),
                    metadatas: meta.metadatas,
                    stream: rx,
                })
            }
            TapHubResponse::Error(e) => Err(e),
            _ => Err(TapHubError::Internal(format!(
                "Unexpected response to RequestAudio: {:?}",
                resp
            ))),
        }
    }

    pub async fn preload_audio(
        &self,
        req: CachedAudioRequest,
    ) -> Result<AudioMetaResponse, TapHubError> {
        match self
            .execute_request(TapHubRequest::PreloadAudio(req))
            .await?
        {
            TapHubResponse::MetaReady(meta) => Ok(meta),
            TapHubResponse::Error(e) => Err(e),
            resp => Err(TapHubError::Internal(format!(
                "Unexpected response to PreloadAudio: {:?}",
                resp
            ))),
        }
    }

    pub async fn request_audio_meta(
        &self,
        req: AudioRequest,
    ) -> Result<AudioMetaResponse, TapHubError> {
        match self
            .execute_request(TapHubRequest::RequestAudioMeta(req))
            .await?
        {
            TapHubResponse::MetaReady(meta) => Ok(meta),
            TapHubResponse::Error(e) => Err(e),
            resp => Err(TapHubError::Internal(format!(
                "Unexpected response to RequestAudioMeta: {:?}",
                resp
            ))),
        }
    }
}

/// The index of the pooled connection the next attempt should use.
///
/// Starts at the round-robin cursor and takes the first connection that has not
/// timed out `WEDGED_AFTER` times in a row. When every connection is wedged the
/// rotation is used anyway: refusing to send would turn a taphub-wide slowdown into
/// an outage of our own.
fn pick_index(start: usize, n: usize, is_wedged: impl Fn(usize) -> bool) -> usize {
    for offset in 0..n {
        let idx = (start + offset) % n;
        if !is_wedged(idx) {
            return idx;
        }
    }
    start
}

/// Why a track's task stopped.
///
/// Logged as `reason=` so a stall in production is distinguishable from a listener
/// that went away and from a clean end of stream — the audit could not tell those
/// apart from the logs, which is part of why the 15-second silence went unexplained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrackEnd {
    /// The transfer ended and playback reached the end of the stream.
    Eof,
    /// The listener dropped the PCM receiver; there was nothing left to send to.
    ConsumerGone,
    /// Nothing arrived for the stall timeout: the source wedged mid-stream.
    Stalled,
    /// A frame could not be decoded.
    DecodeFailure,
    /// The buffer rejected a packet as invalid; the cache was invalidated.
    InvalidPacket,
}

impl TrackEnd {
    fn reason(&self) -> &'static str {
        match self {
            TrackEnd::Eof => "eof",
            TrackEnd::ConsumerGone => "consumer-gone",
            TrackEnd::Stalled => "stalled",
            TrackEnd::DecodeFailure => "decode-failure",
            TrackEnd::InvalidPacket => "invalid-packet",
        }
    }
}

/// The jitter buffer's own numbers, in the shape the report serializes.
fn buffer_view(stats: &BufferStats) -> BufferView {
    BufferView {
        depth: stats.depth,
        capacity: stats.capacity,
        max_lead_frames: stats.max_lead_frames,
        buffered_ms: stats.buffered_ms,
        playhead_ms: stats.playhead_ms,
        inserted: stats.inserted,
        dropped: stats.dropped,
        late: stats.late,
        concealed: stats.concealed,
        stall_events: stats.stall_events,
        last_insert_unix_ms: stats.last_insert_unix_ms,
    }
}

fn is_transport_error(e: &protofish3::Error) -> bool {
    matches!(
        e,
        protofish3::Error::ConnectionClosed(_)
            | protofish3::Error::ChanClosed(_)
            | protofish3::Error::DriverGone
            | protofish3::Error::Quic(_)
            | protofish3::Error::Connect(_)
            | protofish3::Error::Write(_)
            | protofish3::Error::Read(_)
            | protofish3::Error::ReadExact(_)
            | protofish3::Error::Io(_)
            | protofish3::Error::HandshakeTimeout
    )
}

async fn send_invalidate_cache(
    conn: Arc<ReconnectingClient>,
    req: CachedAudioRequest,
    timeout: Duration,
) {
    let result = tokio::time::timeout(timeout, async move {
        let (sender, mut receiver) = match conn.open_chan().await {
            Ok(c) => c,
            Err(_) => {
                tracing::warn!("send_invalidate_cache: failed to open chan");
                return;
            }
        };
        let Ok(payload) = rmp_serde::to_vec(&TapHubRequest::InvalidateCache(req)) else {
            tracing::warn!("send_invalidate_cache: failed to serialize request");
            return;
        };
        if sender.send_msg(payload).await.is_err() {
            tracing::warn!("send_invalidate_cache: failed to send payload");
            return;
        }
        if let Err(e) = receiver.recv_msg().await {
            tracing::warn!("send_invalidate_cache: failed to receive response: {:?}", e);
        }
    })
    .await;

    if result.is_err() {
        tracing::warn!("send_invalidate_cache: timed out");
    }
}

pub fn load_certs<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
    Ok(certs)
}

#[cfg(test)]
mod tests {
    use super::{ClientHealth, pick_index};
    use std::time::Duration;

    /// A wedged connection leaves the rotation, and the rotation is never refused.
    #[test]
    fn pick_index_skips_wedged_connections() {
        assert_eq!(pick_index(0, 4, |_| false), 0);
        assert_eq!(pick_index(0, 4, |i| i == 0), 1);
        // Wraps around the end of the pool.
        assert_eq!(pick_index(3, 4, |i| i == 3), 0);
        assert_eq!(pick_index(0, 4, |i| i < 3), 3);
        // Everything wedged: keep sending rather than fail.
        assert_eq!(pick_index(2, 4, |_| true), 2);
    }

    /// The rebuild trigger is a run of outright failures, and a success in
    /// between is what clears it — not elapsed time, and not a stall.
    #[test]
    fn health_is_wedged_after_a_run_of_failures() {
        let healthy = ClientHealth {
            consecutive_failures: 0,
            since_last_success: Some(Duration::from_secs(2)),
            resets: 0,
        };
        assert!(!healthy.is_wedged(3));

        // One or two failures happen on a taphub restart and then heal.
        let flaky = ClientHealth {
            consecutive_failures: 2,
            since_last_success: Some(Duration::from_secs(20)),
            resets: 1,
        };
        assert!(!flaky.is_wedged(3));

        let wedged = ClientHealth {
            consecutive_failures: 3,
            since_last_success: Some(Duration::from_secs(45)),
            resets: 0,
        };
        assert!(wedged.is_wedged(3));

        // Never answered at all is a wedge once the run is long enough.
        let never = ClientHealth {
            consecutive_failures: 5,
            since_last_success: None,
            resets: 2,
        };
        assert!(never.is_wedged(3));
    }
}
