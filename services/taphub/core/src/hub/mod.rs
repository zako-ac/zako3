use std::{
    collections::HashMap,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use parking_lot::Mutex;
use tokio::sync::watch;
use zako3_types::{OnlineTapState, OnlineTapStates, hq::TapId};
use zakofish_taphub::{ZakofishError, create_server_config, hub::ZakofishHub};

use zako3_preload_cache::AudioCache;

use crate::{app::App, routing::DynamicSampler};
use zako3_metrics::TapRedisMetrics;
use zako3_states::{RedisPubSub, TapHubStateService};

pub mod connection;
pub use connection::TapHubConnectionHandler;

/// A single live tap connection held by this taphub instance.
///
/// The registry of these entries is the authoritative source of truth for which
/// connections exist — routing reads it directly (never Redis), so it cannot
/// drift. Redis holds only a TTL-leased projection for cross-service consumers.
pub(crate) struct ConnEntry {
    pub(crate) state: OnlineTapState,
    pub(crate) disconnect_tx: watch::Sender<bool>,
    /// The tap has said it is going away: still connected, but no longer a
    /// candidate for a request.
    ///
    /// Kept as a field rather than by removing the entry, because the entry is
    /// also the bookkeeping for uptime, metrics and the Redis projection, and a
    /// removed entry makes the later `on_tap_disconnected` look like a
    /// connection that was never here. Flipping this makes the connection
    /// unroutable at once and leaves the rest of the teardown to its normal path.
    pub(crate) draining: bool,
    /// Requests this connection has failed to answer in a row, with no answer in
    /// between. Reset by any answer, so a connection that is merely slow under
    /// load keeps its place while one that has stopped answering does not.
    pub(crate) unanswered: u32,
    /// While this is in the future the connection is out of the routing set: it
    /// is still connected and still in the registry, but nothing is sent to it.
    /// A cooldown rather than a permanent exclusion, because a connection that
    /// recovers has to be able to prove it (see 2026-11-18 05:53, the same
    /// trade-off in the transport client's pool).
    pub(crate) suspect_until: Option<Instant>,
    /// When this connection last answered a request. An answer is proof the tap
    /// is alive, which is what keeps a busy tap out of the suspect path.
    pub(crate) last_answered_at: Option<Instant>,
    /// Retirements in a row without an answer in between. Drives the escalating
    /// cooldown: a connection that is wedged comes back less and less often,
    /// while one that recovers is back in rotation after a single answer.
    pub(crate) suspect_strikes: u32,
    /// Request chans opened on this connection that no answer has closed yet.
    ///
    /// Maintained where the request is actually made and answered, and published
    /// as `taphub_tap_conn_inflight_requests`. protofish3 has no chan-close, so a
    /// request abandoned on timeout leaves its chan open on the wire: this is the
    /// only direct measurement of that, and with the cumulative opened/closed
    /// counters it is what decides the stream-credit hypothesis.
    pub(crate) inflight_requests: i64,
    /// The hub has decided this connection is dead and is closing it
    /// (`TapHub::self_heal_heartbeat`).
    ///
    /// A self-heal close is neither a drain nor an accident, and the two flags
    /// are kept apart on purpose: `draining` means the tap said it was leaving
    /// and is counted as a planned departure, while this one means the hub
    /// found a connection that had stopped answering and shut it. From the
    /// moment it is set the connection is out of the routing set, so the window
    /// between deciding to close and the disconnect landing cannot swallow a
    /// request.
    pub(crate) self_healing: bool,
}

impl ConnEntry {
    /// Whether this connection should be left out of the routing set right now.
    fn suspect(&self, now: Instant) -> bool {
        self.suspect_until.is_some_and(|until| until > now)
    }

    /// How long this connection has gone without answering anything.
    ///
    /// The number the eviction rule is written in, computed in one place so the
    /// retirement decision, its log line and the published gauge cannot drift
    /// apart. A connection that has never answered is as silent as the rule
    /// treats it, which is [`SILENT_FOR`].
    fn silent_for(&self, now: Instant) -> Duration {
        self.last_answered_at
            .map_or(SILENT_FOR, |at| now.saturating_duration_since(at))
    }
}

/// Consecutive requests a connection may fail to answer before it is taken out
/// of the routing set. One is too few: under saturation a busy tap times out
/// requests and answers others, and evicting on a single timeout would empty the
/// routing set exactly when it is needed. Three is roughly the window in which
/// the load test showed a buried tap still answering something.
pub(crate) const UNANSWERED_EVICT_AFTER: u32 = 3;

/// How long a connection that failed to answer is kept out of the routing set
/// before it is allowed to take a request again. Long enough for a stale
/// connection to be noticed and dropped by the transport, short enough that a
/// tap which recovers is back in rotation within a request or two.
pub(crate) const SUSPECT_COOLDOWN: Duration = Duration::from_secs(60);

/// The longest a connection that keeps failing is kept out of the routing set.
pub(crate) const SUSPECT_COOLDOWN_MAX: Duration = Duration::from_secs(15 * 60);

/// How long a connection that failed to answer is kept out of the routing set,
/// given how many times in a row it has been retired without answering anything
/// in between.
///
/// The first retirement is [`SUSPECT_COOLDOWN`], and every further one doubles
/// it up to [`SUSPECT_COOLDOWN_MAX`]. A flat cooldown is a recovery probe that
/// never stops: a permanently wedged connection returns every minute and
/// swallows roughly half the requests whenever there is one healthy replica
/// beside it — which is what "Zako reads messages selectively" looks like from
/// the outside. Escalating means the wedged connection gets one probe every
/// quarter of an hour instead of every minute, and any answer puts it straight
/// back.
pub(crate) fn suspect_cooldown(strikes: u32) -> Duration {
    let doublings = strikes.saturating_sub(1).min(4);
    (SUSPECT_COOLDOWN * (1u32 << doublings)).min(SUSPECT_COOLDOWN_MAX)
}

/// How long a connection must have gone without answering anything before it
/// can be retired. A tap that is merely busy answers something inside this
/// window (the load test's saturated tap answered every few seconds), so it is
/// never retired; a connection whose peer is gone answers nothing at all.
pub(crate) const SILENT_FOR: Duration = Duration::from_secs(20);

/// How often the per-connection gauges are republished.
///
/// Silence is the number that changes when nothing happens, so a gauge sampled
/// only at events would freeze at the last event's value on exactly the
/// connection that has gone quiet — the case the number exists for. Five seconds
/// is comfortably inside a 15s scrape without taking the registry lock often.
pub(crate) const CONNECTION_GAUGE_INTERVAL: Duration = Duration::from_secs(5);

/// Whether a connection that failed `unanswered` requests in a row, having been
/// silent for `silent_for`, should be retired from the routing set.
///
/// Both halves are needed: consecutive failures alone retire a tap that is
/// simply buried under a burst, and silence alone retires a tap that has had no
/// traffic in minutes the moment one request misses.
pub(crate) fn should_retire(unanswered: u32, silent_for: Duration) -> bool {
    unanswered >= UNANSWERED_EVICT_AFTER && silent_for >= SILENT_FOR
}

/// How long a connection that has already been retired once may stay silent
/// before the hub closes it instead of merely routing around it.
///
/// Retirement alone is not recovery: the cooldown lapses, the connection comes
/// back into the routing set, swallows the next request, and is retired again —
/// forever, because nothing on this side ever tells the tap its connection is
/// worthless. Five minutes is long enough to rule out a tap that is merely
/// buried (the load test's saturated tap answered every few seconds) and short
/// enough that a wedged connection is gone before the escalating cooldown
/// reaches its fifteen-minute ceiling.
pub(crate) const SELF_HEAL_SILENT_FOR: Duration = Duration::from_secs(300);

/// Whether a connection that has been retired at least once, and has been silent
/// for `silent_for`, should be closed rather than only kept out of routing.
///
/// Both halves are required, exactly as in [`should_retire`], and the strike
/// half is the load-bearing one: measured at 03:00 on 2026-11-27 every idle
/// connection of the default tap had been silent for 2050–2075 seconds with
/// zero strikes, and a rule written on silence alone would have closed all ten
/// of them every five minutes all night, for no reason, training everyone to
/// ignore the alert.
pub(crate) fn should_self_heal(strikes: u32, silent_for: Duration) -> bool {
    strikes > 0 && silent_for >= SELF_HEAL_SILENT_FOR
}

/// How often the self-heal sweep looks for connections to close.
///
/// The number only bounds how long a wedged connection survives past the
/// threshold; closing is idempotent per connection (the entry is marked before
/// the close is sent), so a tick that overlaps a teardown does nothing.
pub(crate) const SELF_HEAL_SWEEP_INTERVAL: Duration = Duration::from_secs(30);

/// The close code the hub sends when it self-heals a connection.
///
/// In the application-defined range (4000–4999) so it cannot be confused with
/// the `0` a draining tap sends, and it lands in `tap_conn_closed_total` and the
/// tap's own log as a number a reader can look up.
pub(crate) const SELF_HEAL_CLOSE_CODE: u64 = 4100;

pub(crate) type ConnectionRegistry = Arc<Mutex<HashMap<u64, ConnEntry>>>;

pub struct TapHub {
    pub zf_hub: ZakofishHub,
    pub sampler: Arc<Mutex<DynamicSampler>>,
    pub state_service: TapHubStateService,
    pub metrics_service: TapRedisMetrics,
    pub app: App,
    /// Cache client. Today this is the HTTP-backed [`zako3_cache_client::RemoteAudioCache`];
    /// the trait indirection lets tests inject an in-memory impl.
    pub audio_cache: Arc<dyn AudioCache>,
    pub request_timeout: Duration,
    pub history_pubsub: Arc<RedisPubSub>,
    pub(crate) connections: ConnectionRegistry,
}

impl TapHub {
    pub async fn new(
        app: App,
        bind_address: &str,
        cert_file: impl AsRef<Path>,
        key_file: impl AsRef<Path>,
        audio_cache: Arc<dyn AudioCache>,
        request_timeout_ms: u64,
        history_pubsub: Arc<RedisPubSub>,
    ) -> Result<Self, ZakofishError> {
        let cert_file = cert_file.as_ref().to_path_buf();
        let key_file = key_file.as_ref().to_path_buf();

        let server_config = create_server_config(
            bind_address.parse().map_err(|_| {
                ZakofishError::ProtocolError("Invalid bind address is provided".to_string())
            })?,
            &cert_file,
            &key_file,
        )?;

        let connections: ConnectionRegistry = Arc::new(Mutex::new(HashMap::new()));

        let handler = TapHubConnectionHandler {
            app: app.clone(),
            state_service: app.tap_state_service.clone(),
            metrics_service: app.tap_metrics_service.clone(),
            connections: Arc::clone(&connections),
        };

        let zf_hub = ZakofishHub::new(server_config, Arc::new(handler))?;

        Ok(Self {
            zf_hub,
            sampler: Arc::new(Mutex::new(DynamicSampler::new())),
            state_service: app.tap_state_service.clone(),
            metrics_service: app.tap_metrics_service.clone(),
            app,
            audio_cache,
            request_timeout: Duration::from_millis(request_timeout_ms),
            history_pubsub,
            connections,
        })
    }

    pub async fn run(&self) -> Result<(), ZakofishError> {
        tokio::select! {
            r = self.zf_hub.run() => r,
            _ = self.lease_heartbeat() => Ok(()),
            _ = self.connection_gauge_heartbeat() => Ok(()),
            _ = self.self_heal_heartbeat() => Ok(()),
        }
    }

    /// Close connections that retired and then stayed silent.
    ///
    /// This is the half of the eviction design that was missing: retiring a
    /// connection takes it out of routing, but the cooldown lapses, the
    /// connection returns, swallows a request, and is retired again — and
    /// nothing ever tells the tap that its connection is worthless. Closing it
    /// does: the tap's transport sees CONNECTION_CLOSE, reconnects, and
    /// re-handshakes, which is a fresh connection on a fresh socket. The
    /// 2026-11-27 00:25 incident is the proof this works — restarting taphub
    /// closed the wedged connection and the tap came back and answered inside
    /// 90 seconds; this does the same thing to one connection instead of all of
    /// them.
    ///
    /// The registry entry is marked before the close is sent, so a request
    /// cannot be routed into the gap between the decision and the teardown.
    async fn self_heal_heartbeat(&self) {
        let mut ticker = tokio::time::interval(SELF_HEAL_SWEEP_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;

            let candidates = {
                let mut guard = self.connections.lock();
                take_self_heal_candidates(&mut guard, Instant::now())
            };

            for (connection_id, tap_id, silent_for, strikes) in candidates {
                tracing::warn!(
                    tap_id = %tap_id.0,
                    connection_id,
                    silent_secs = silent_for.as_secs(),
                    strikes,
                    close_code = SELF_HEAL_CLOSE_CODE,
                    "closing a tap connection that has stopped answering — self-heal"
                );
                let closed = self
                    .zf_hub
                    .close_connection(&tap_id, connection_id, SELF_HEAL_CLOSE_CODE)
                    .await;
                if !closed {
                    // The teardown won the race; `on_tap_disconnected` will do
                    // the bookkeeping, and the entry is on its way out anyway.
                    tracing::info!(
                        tap_id = %tap_id.0,
                        connection_id,
                        "self-heal close found no live session; the connection was already gone"
                    );
                }
            }
        }
    }

    /// Republish every live connection's silence, strikes and open chans.
    ///
    /// The values are read under the registry lock into a small snapshot and
    /// recorded outside it, so the lock is never held across the meter.
    async fn connection_gauge_heartbeat(&self) {
        let mut ticker = tokio::time::interval(CONNECTION_GAUGE_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        // Every tap this process has held a connection for, so a tap whose last
        // connection went away is published as `0` instead of its series
        // vanishing. A missing series and a zero are different claims: the alert
        // this number exists for ("the default tap has no connection") must be
        // able to tell "all its connections are gone" from "nothing scraped".
        let mut seen_taps: std::collections::HashSet<TapId> = std::collections::HashSet::new();

        loop {
            ticker.tick().await;

            let now = Instant::now();
            let (snapshot, live_per_tap): (Vec<(u64, TapId, f64, u64, i64)>, HashMap<TapId, u64>) = {
                let guard = self.connections.lock();
                let mut live: HashMap<TapId, u64> = HashMap::new();
                for entry in guard.values() {
                    if entry.draining || entry.self_healing {
                        continue;
                    }
                    *live.entry(entry.state.tap_id.clone()).or_insert(0) += 1;
                }
                let snapshot = guard
                    .values()
                    .map(|entry| {
                        (
                            entry.state.connection_id,
                            entry.state.tap_id.clone(),
                            entry.silent_for(now).as_secs_f64(),
                            u64::from(entry.suspect_strikes),
                            entry.inflight_requests,
                        )
                    })
                    .collect();
                (snapshot, live)
            };

            let metrics = crate::metrics::metrics();
            for (connection_id, tap_id, silent_secs, strikes, inflight) in snapshot {
                let key = crate::metrics::connection_key_values(connection_id, &tap_id.0);
                metrics.tap_conn_silent_seconds.record(silent_secs, &key);
                metrics.tap_conn_strikes.record(strikes, &key);
                metrics.tap_conn_inflight_requests.record(inflight, &key);
            }

            seen_taps.extend(live_per_tap.keys().cloned());
            for tap_id in &seen_taps {
                let connected = live_per_tap.get(tap_id).copied().unwrap_or(0);
                metrics
                    .tap_connected
                    .record(connected, &[opentelemetry::KeyValue::new("tap_id", tap_id.0.to_string())]);
            }
        }
    }

    /// Periodically re-publish every locally-held connection's state to Redis so
    /// its TTL lease never lapses while the connection is live. The registry is
    /// the source of truth; this only refreshes the projection. On a hard crash
    /// the loop stops, the keys expire, and stale "online" state disappears.
    async fn lease_heartbeat(&self) {
        let interval_secs = (self.state_service.lease_ttl_secs() / 3).max(1);
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;

            // Snapshot the registry grouped by tap, dropping the lock before any await.
            let by_tap: HashMap<TapId, OnlineTapStates> = {
                let guard = self.connections.lock();
                let mut map: HashMap<TapId, OnlineTapStates> = HashMap::new();
                for entry in guard.values() {
                    if entry.draining {
                        // A draining tap is not an offering. Refreshing its
                        // lease would advertise it to every reader of the
                        // projection for as long as the drain lasts.
                        continue;
                    }
                    map.entry(entry.state.tap_id.clone())
                        .or_default()
                        .push(entry.state.clone());
                }
                map
            };

            for (tap_id, states) in by_tap {
                if let Err(e) = self
                    .state_service
                    .publish_tap_states(&tap_id, &states)
                    .await
                {
                    tracing::warn!(%e, tap_id = %tap_id.0, "Failed to refresh tap connection lease");
                }
            }
        }
    }

    /// Select an available connection for a tap.
    ///
    /// Reads the in-memory registry directly — the authoritative set of live
    /// connections — so a connection that has already been dropped is never
    /// handed out. The sampler picks a connection, then we subscribe to that
    /// connection's disconnect signal under the same logical view.
    ///
    /// A connection that has announced a drain is skipped for exactly that
    /// reason: it is still connected and may still be answering, but a request
    /// handed to it is a request that has to be retried elsewhere a moment
    /// later. This is the "route new requests to another pod" half of the
    /// scale-down contract, and it needs no cooperation from the pod that is
    /// leaving.
    pub(crate) async fn select_connection(
        &self,
        tap_id: &TapId,
    ) -> Result<(u64, watch::Receiver<bool>), zako3_types::TapHubError> {
        self.select_connection_excluding(tap_id, &[]).await
    }

    /// [`TapHub::select_connection`], but never handing back one of `exclude`.
    ///
    /// The retry in the request path uses this: the connection it just failed on
    /// is excluded before the routing filter is applied, so the second attempt
    /// cannot land on the same connection even if the eviction threshold has not
    /// been reached yet.
    pub(crate) async fn select_connection_excluding(
        &self,
        tap_id: &TapId,
        exclude: &[u64],
    ) -> Result<(u64, watch::Receiver<bool>), zako3_types::TapHubError> {
        let guard = self.connections.lock();

        let mut available: OnlineTapStates = routable_states(&guard, tap_id, Instant::now());
        available.retain(|s| !exclude.contains(&s.connection_id));

        let connection_id = self
            .sampler
            .lock()
            .next_connection_id(&available)
            .ok_or_else(|| {
                // Nothing left to route to. The caller gets `TapUnavailable`,
                // which HQ classifies as `tap_unavailable` — a retriable reason,
                // so the message is retried once and then reported to the user
                // instead of being dropped in silence. `held` separates "every
                // replica is draining" (held > 0, nothing routable) from "this
                // tap was never here" (held == 0), which is the difference
                // between a scale-down that caught a request and a tap that is
                // simply down.
                let held = guard.values().filter(|e| &e.state.tap_id == tap_id).count();
                // Held-but-none-routable means every replica is on its way out,
                // which is a scheduled scale-down and says nothing new at warn;
                // held == 0 means the tap is simply gone, which does.
                let message = "no connection available for tap";
                if held == available.len() {
                    tracing::warn!(tap_id = %tap_id.0, held, "{message}");
                } else {
                    tracing::debug!(
                        tap_id = %tap_id.0,
                        held,
                        draining = held - available.len(),
                        "{message}"
                    );
                }
                crate::metrics::metrics().no_tap_available_total.add(
                    1,
                    &[
                        opentelemetry::KeyValue::new("tap_id", tap_id.0.to_string()),
                        opentelemetry::KeyValue::new("held", held.to_string()),
                    ],
                );
                zako3_types::TapHubError::TapUnavailable
            })?;

        let disconnect_rx = guard
            .get(&connection_id)
            .map(|e| e.disconnect_tx.subscribe())
            .ok_or(zako3_types::TapHubError::TapUnavailable)?;

        Ok((connection_id, disconnect_rx))
    }

    /// Record that this connection did not answer a request.
    ///
    /// After [`UNANSWERED_EVICT_AFTER`] in a row with no answer in between **and**
    /// a full [`SILENT_FOR`] with nothing from it, the connection leaves the
    /// routing set for [`SUSPECT_COOLDOWN`]. This is the piece that turns a stale
    /// connection from an eight-minute blackout into a handful of failed
    /// requests: the tap is still connected as far as the transport knows, so
    /// nothing else in the system would ever stop routing to it.
    ///
    /// The [`SILENT_FOR`] half is what keeps a *busy* tap from being retired. A
    /// tap under load answers one request while three others time out waiting
    /// behind it, so consecutive timeouts alone would empty the routing set
    /// exactly when it is needed; an answer resets the clock and nothing is
    /// retired while the tap is demonstrably still talking.
    pub(crate) fn mark_unanswered(&self, tap_id: &TapId, connection_id: u64) {
        let now = Instant::now();
        let mut guard = self.connections.lock();
        let Some(entry) = guard.get_mut(&connection_id) else {
            return;
        };
        entry.unanswered = entry.unanswered.saturating_add(1);
        let silent_for = entry.silent_for(now);
        // Published per connection, from the same numbers the decision below is
        // made from: a connection that keeps being retired while it stays silent
        // beside a neighbour that answers is S2b, and until now that was one log
        // line per retirement and nothing a dashboard could read.
        let key = crate::metrics::connection_key_values(connection_id, &tap_id.0);
        crate::metrics::metrics()
            .tap_conn_silent_seconds
            .record(silent_for.as_secs_f64(), &key);
        if !should_retire(entry.unanswered, silent_for) {
            return;
        }
        // Log once per retirement, not once per failed request: a retired
        // connection keeps failing until it is allowed back, and one line per
        // failure buries the rest of the log.
        let already_out = entry.suspect(now);
        entry.suspect_strikes = entry.suspect_strikes.saturating_add(1);
        let cooldown = suspect_cooldown(entry.suspect_strikes);
        entry.suspect_until = Some(now + cooldown);
        crate::metrics::metrics()
            .tap_conn_strikes
            .record(u64::from(entry.suspect_strikes), &key);
        if already_out {
            return;
        }
        tracing::warn!(
            tap_id = %tap_id.0,
            connection_id,
            unanswered = entry.unanswered,
            silent_secs = silent_for.as_secs(),
            cooldown_secs = cooldown.as_secs(),
            strikes = entry.suspect_strikes,
            "tap connection is not answering — taken out of the routing set"
        );
        crate::metrics::metrics().tap_conn_unresponsive_total.add(
            1,
            &[opentelemetry::KeyValue::new("tap_id", tap_id.0.to_string())],
        );
    }

    /// Whether this connection is on probation: it has been retired for not
    /// answering at least once since it last answered anything.
    ///
    /// The metadata path uses this to give it half the request budget instead of
    /// all of it, so a second connection can be tried inside the same request.
    /// A connection that answers anything at all is off probation immediately.
    pub(crate) fn is_on_probation(&self, connection_id: u64) -> bool {
        self.connections
            .lock()
            .get(&connection_id)
            .is_some_and(|entry| entry.suspect_strikes > 0)
    }

    /// Whether this metadata attempt should be held silent by the stall probe
    /// (`ZAKO_META_STALL_INJECT`, see `handler/stall_inject.rs`), and if so, put
    /// the connection on probation.
    ///
    /// The retry this exists to exercise only happens on the probation path, so
    /// waiting for three real unanswered requests plus a silence window would
    /// make the counter provable only during an incident. Nothing calls this
    /// with the probe unarmed, and then it returns false on the first line.
    pub(crate) fn stall_metadata_for(&self, tap_id: &TapId, connection_id: u64) -> bool {
        let Some(probe) = super::handler::stall_probe() else {
            return false;
        };
        if probe.tap_id != tap_id.0 {
            return false;
        }

        let mut guard = self.connections.lock();
        let ids: Vec<u64> = guard
            .values()
            .filter(|e| &e.state.tap_id == tap_id)
            .map(|e| e.state.connection_id)
            .collect();
        if !is_within_oldest(&ids, probe.connections, connection_id) {
            return false;
        }
        if let Some(entry) = guard
            .get_mut(&connection_id)
            .filter(|entry| entry.suspect_strikes == 0)
        {
            entry.suspect_strikes = 1;
            tracing::warn!(
                tap_id = %entry.state.tap_id.0,
                connection_id,
                "tap connection entered probation on the stall probe — its metadata requests now get half the budget"
            );
        }
        true
    }

    /// Record that this connection answered, which puts it back in normal
    /// rotation whatever it accumulated before.
    pub(crate) fn mark_answered(&self, connection_id: u64) {
        let mut guard = self.connections.lock();
        let Some(entry) = guard.get_mut(&connection_id) else {
            return;
        };
        let strikes = entry.suspect_strikes;
        entry.unanswered = 0;
        entry.suspect_until = None;
        entry.suspect_strikes = 0;
        entry.last_answered_at = Some(Instant::now());
        if strikes > 0 {
            // The other half of the probation marker: a connection that answers
            // is trusted again at once, and the halved budget goes with it.
            tracing::info!(
                tap_id = %entry.state.tap_id.0,
                connection_id,
                strikes,
                "tap connection left probation — it answered, so it gets the full request budget again"
            );
            let key = crate::metrics::connection_key_values(connection_id, &entry.state.tap_id.0);
            crate::metrics::metrics()
                .tap_conn_strikes
                .record(0, &key);
        }
    }

    /// Record one request chan opened on this connection.
    ///
    /// Called where the tap is actually asked, which is one chan per call:
    /// `ZakofishHub::request_audio` and `request_audio_metadata` each open
    /// exactly one (`conn.open_chan()`), so counting at the call site counts
    /// chans.
    pub(crate) fn record_stream_opened(&self, tap_id: &TapId, connection_id: u64) {
        crate::metrics::record_stream_opened(connection_id, &tap_id.0);
        let key = crate::metrics::connection_key_values(connection_id, &tap_id.0);
        if let Some(entry) = self.connections.lock().get_mut(&connection_id) {
            entry.inflight_requests = entry.inflight_requests.saturating_add(1);
            crate::metrics::metrics()
                .tap_conn_inflight_requests
                .record(entry.inflight_requests, &key);
        }
    }

    /// Record that the chan a request opened on this connection got an answer.
    ///
    /// An answer of any kind closes it, including a tap that refuses the
    /// request: what does not close it is a timeout, a transport error or a
    /// disconnect, because protofish3 has no chan-close and those leave the chan
    /// open on the wire. That asymmetry is the leak the gauge measures.
    pub(crate) fn record_stream_closed(&self, tap_id: &TapId, connection_id: u64) {
        crate::metrics::record_stream_closed(connection_id, &tap_id.0);
        let key = crate::metrics::connection_key_values(connection_id, &tap_id.0);
        if let Some(entry) = self.connections.lock().get_mut(&connection_id) {
            entry.inflight_requests = entry.inflight_requests.saturating_sub(1);
            crate::metrics::metrics()
                .tap_conn_inflight_requests
                .record(entry.inflight_requests, &key);
        }
    }
}

/// The connections the self-heal sweep must close right now, marked as closing.
///
/// Marking is part of taking: the entry leaves the routing set in the same
/// critical section that selects it, so nothing can be dispatched into the
/// window between "the hub decided this connection is dead" and "the close
/// lands". A second call returns nothing for the same connection, which is what
/// makes an overlapping sweep tick harmless.
///
/// Draining connections are skipped: they are already on their way out, and
/// replacing a tap's own clean departure with a hub-side close would lose the
/// drain accounting for no gain.
pub(crate) fn take_self_heal_candidates(
    registry: &mut HashMap<u64, ConnEntry>,
    now: Instant,
) -> Vec<(u64, TapId, Duration, u32)> {
    let mut out = Vec::new();
    for entry in registry.values_mut() {
        if entry.draining || entry.self_healing {
            continue;
        }
        let silent_for = entry.silent_for(now);
        if !should_self_heal(entry.suspect_strikes, silent_for) {
            continue;
        }
        entry.self_healing = true;
        out.push((
            entry.state.connection_id,
            entry.state.tap_id.clone(),
            silent_for,
            entry.suspect_strikes,
        ));
    }
    out
}

/// Whether `connection_id` is one of the `n` oldest connection ids in `ids`.
///
/// The stall probe names connections this way because arming it restarts the
/// pod, which renumbers every connection: an id read before the restart is
/// meaningless after it, while "the first two connections of this tap" survives
/// the reconnect.
pub(crate) fn is_within_oldest(ids: &[u64], n: usize, connection_id: u64) -> bool {
    let mut sorted = ids.to_vec();
    sorted.sort_unstable();
    sorted.truncate(n);
    sorted.contains(&connection_id)
}

/// The connections of one tap that a request may be routed to: live, not on
/// their way out, and not currently suspect.
pub(crate) fn routable_states(
    registry: &std::collections::HashMap<u64, ConnEntry>,
    tap_id: &TapId,
    now: Instant,
) -> OnlineTapStates {
    registry
        .values()
        .filter(|e| {
            &e.state.tap_id == tap_id && !e.draining && !e.self_healing && !e.suspect(now)
        })
        .map(|e| e.state.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use zako3_types::TapName;

    fn entry(connection_id: u64, draining: bool) -> ConnEntry {
        let (disconnect_tx, _rx) = watch::channel(false);
        ConnEntry {
            state: OnlineTapState {
                tap_id: TapId("t1".to_string()),
                tap_name: TapName("test".to_string()),
                connection_id,
                friendly_name: "test".to_string(),
                selection_weight: 1.0,
                connected_at: Utc::now(),
                replica_id: String::new(),
            },
            disconnect_tx,
            draining,
            unanswered: 0,
            suspect_until: None,
            last_answered_at: Some(Instant::now()),
            suspect_strikes: 0,
            inflight_requests: 0,
            self_healing: false,
        }
    }

    fn registry(entries: Vec<ConnEntry>) -> std::collections::HashMap<u64, ConnEntry> {
        entries
            .into_iter()
            .map(|e| (e.state.connection_id, e))
            .collect()
    }

    fn ids(states: &OnlineTapStates) -> Vec<u64> {
        let mut ids: Vec<u64> = states.iter().map(|s| s.connection_id).collect();
        ids.sort_unstable();
        ids
    }

    /// The routing set as the request path reads it.
    fn routable(reg: &std::collections::HashMap<u64, ConnEntry>, tap: &TapId) -> Vec<u64> {
        ids(&routable_states(reg, tap, Instant::now()))
    }

    #[test]
    fn a_draining_connection_leaves_the_routing_set_at_once() {
        let tap = TapId("t1".to_string());
        let mut reg = registry(vec![entry(1, false), entry(2, false)]);
        assert_eq!(routable(&reg, &tap), vec![1, 2]);

        // This is the whole of the routing change: the flag the drain handler
        // flips, read by the same function the request path uses.
        reg.get_mut(&1).unwrap().draining = true;
        assert_eq!(
            routable(&reg, &tap),
            vec![2],
            "a draining connection must not be selectable"
        );
    }

    #[test]
    fn the_last_tap_draining_leaves_nothing_to_route_to() {
        let tap = TapId("t1".to_string());
        let mut reg = registry(vec![entry(1, false)]);
        reg.get_mut(&1).unwrap().draining = true;
        assert!(routable(&reg, &tap).is_empty());
    }

    #[test]
    fn draining_one_tap_does_not_touch_another() {
        let mut other = entry(9, false);
        other.state.tap_id = TapId("t2".to_string());
        let tap = TapId("t1".to_string());
        let reg = registry(vec![entry(1, true), other]);
        assert!(routable(&reg, &tap).is_empty());
        assert_eq!(routable(&reg, &TapId("t2".to_string())), vec![9]);
    }

    /// A connection that stopped answering takes its place out of the routing
    /// set, and the connection that is still answering keeps its own. This is
    /// the scale-down case the load test caught: the surviving pod was fine, and
    /// every request still went to a connection whose peer was gone.
    #[test]
    fn an_unanswered_connection_leaves_the_routing_set() {
        let tap = TapId("t1".to_string());
        let mut reg = registry(vec![entry(1, false), entry(2, false)]);

        reg.get_mut(&1).unwrap().unanswered = UNANSWERED_EVICT_AFTER - 1;
        assert_eq!(routable(&reg, &tap), vec![1, 2], "not yet at the threshold");

        reg.get_mut(&1).unwrap().unanswered = UNANSWERED_EVICT_AFTER;
        reg.get_mut(&1).unwrap().suspect_until = Some(Instant::now() + SUSPECT_COOLDOWN);
        assert_eq!(
            routable(&reg, &tap),
            vec![2],
            "a connection that stopped answering must not be routed to"
        );
    }

    /// The exclusion is a cooldown, not a life sentence: once it lapses the
    /// connection is a candidate again and can prove itself with one request.
    #[test]
    fn a_suspect_connection_returns_after_the_cooldown() {
        let tap = TapId("t1".to_string());
        let mut reg = registry(vec![entry(1, false), entry(2, false)]);
        reg.get_mut(&1).unwrap().suspect_until =
            Some(Instant::now() - std::time::Duration::from_secs(1));
        assert_eq!(routable(&reg, &tap), vec![1, 2]);
    }

    /// An answer clears the count, so a busy tap that times out some requests
    /// and answers others is never mistaken for a dead one — and it takes the
    /// connection off probation, so the next request gets the full budget again.
    #[test]
    fn a_single_answer_clears_the_unanswered_count() {
        let mut e = entry(1, false);
        e.unanswered = UNANSWERED_EVICT_AFTER + 5;
        e.suspect_until = Some(Instant::now() + SUSPECT_COOLDOWN);
        e.suspect_strikes = 4;
        // What `mark_answered` does to the entry.
        e.unanswered = 0;
        e.suspect_until = None;
        e.suspect_strikes = 0;
        assert_eq!(e.unanswered, 0);
        assert_eq!(e.suspect_strikes, 0, "an answer ends probation");
        assert!(!e.suspect(Instant::now()));
    }

    /// A tap buried under a burst fails several requests in a row while still
    /// answering others. It must not be retired: the H-3 load test saturated a
    /// healthy tap this way for minutes at a time.
    #[test]
    fn a_busy_tap_is_not_retired_while_it_still_answers() {
        assert!(!should_retire(
            UNANSWERED_EVICT_AFTER + 9,
            SILENT_FOR - std::time::Duration::from_secs(1)
        ));
    }

    /// A connection that is wedged for good is probed less and less often, so a
    /// healthy neighbour beside it is the one that gets the work. A flat
    /// one-minute cooldown kept handing it every other message.
    #[test]
    fn a_connection_that_keeps_failing_is_retired_for_longer_each_time() {
        assert_eq!(suspect_cooldown(1), SUSPECT_COOLDOWN);
        assert_eq!(suspect_cooldown(2), SUSPECT_COOLDOWN * 2);
        assert_eq!(suspect_cooldown(3), SUSPECT_COOLDOWN * 4);
        assert_eq!(suspect_cooldown(9), SUSPECT_COOLDOWN_MAX);
        assert_eq!(
            suspect_cooldown(u32::MAX),
            SUSPECT_COOLDOWN_MAX,
            "the cooldown must not overflow into something shorter"
        );
    }

    /// A connection whose peer is gone fails and never answers again, so it is
    /// retired as soon as both halves of the rule hold.
    /// The silence the retirement rule reads is the same silence the gauge
    /// publishes: measured from the last answer, so a connection retired at 20
    /// seconds reports 20 and not the cost of one lock.
    #[test]
    fn silence_is_measured_from_the_last_answer() {
        let mut e = entry(1, false);
        let answered_at = Instant::now();
        e.last_answered_at = Some(answered_at);

        assert_eq!(e.silent_for(answered_at).as_secs(), 0);
        assert_eq!(
            e.silent_for(answered_at + std::time::Duration::from_secs(42)).as_secs(),
            42
        );
    }

    #[test]
    fn a_connection_that_stopped_answering_is_retired() {
        assert!(should_retire(UNANSWERED_EVICT_AFTER, SILENT_FOR));
        assert!(!should_retire(UNANSWERED_EVICT_AFTER - 1, SILENT_FOR * 10));
    }

    /// The stall probe writes `suspect_strikes` without a cooldown, so the
    /// connection has to stay routable — otherwise the retry it exists to
    /// exercise could never happen — and one answer has to put it back.
    #[test]
    fn a_connection_on_probation_stays_routable_until_it_answers() {
        let tap = TapId("t1".to_string());
        let mut reg = registry(vec![entry(1, false), entry(2, false)]);

        reg.get_mut(&1).unwrap().suspect_strikes = 1;
        assert!(
            !reg.get(&1).unwrap().suspect(Instant::now()),
            "probation is not retirement: the retry has to be able to land here"
        );
        assert_eq!(routable(&reg, &tap), vec![1, 2]);

        // What `mark_answered` does: the answer ends probation.
        let e = reg.get_mut(&1).unwrap();
        e.suspect_strikes = 0;
        e.last_answered_at = Some(Instant::now());
        assert_eq!(routable(&reg, &tap), vec![1, 2]);
    }

    /// The stall probe picks connections by position, not by the id it was armed
    /// with: the arm restarts the pod, so ids captured before it are gone.
    #[test]
    fn the_stall_probe_picks_the_oldest_connections() {
        let ids = [9u64, 3, 7];
        assert!(is_within_oldest(&ids, 1, 3), "the lowest id is the oldest");
        assert!(!is_within_oldest(&ids, 1, 7));
        assert!(is_within_oldest(&ids, 2, 7));
        assert!(!is_within_oldest(&ids, 2, 9));
        assert!(
            is_within_oldest(&ids, 3, 9),
            "asking for every connection targets every connection"
        );
        assert!(!is_within_oldest(&ids, 1, 42), "an unknown id is never one");
        assert!(!is_within_oldest(&[], 5, 3));
    }

    /// The rule that decides whether a connection is closed rather than only
    /// routed around. Silence alone is not enough: measured at 03:00 on
    /// 2026-11-27, ten idle connections of the default tap had been silent for
    /// 2050–2075 seconds with zero strikes, and that is a quiet night, not a
    /// fault.
    #[test]
    fn silence_alone_never_closes_a_connection() {
        assert!(should_self_heal(1, SELF_HEAL_SILENT_FOR));
        assert!(should_self_heal(3, SELF_HEAL_SILENT_FOR * 4));
        assert!(!should_self_heal(0, SELF_HEAL_SILENT_FOR * 100));
        assert!(
            !should_self_heal(1, SELF_HEAL_SILENT_FOR - std::time::Duration::from_secs(1)),
            "the threshold is a floor, not a suggestion"
        );
    }

    /// The sweep takes the wedged connection, leaves the quiet neighbour alone,
    /// and takes the wedged one out of routing in the same step — otherwise a
    /// request lands on it between the decision and the close.
    #[test]
    fn the_sweep_closes_the_wedged_connection_and_leaves_the_quiet_one() {
        let tap = TapId("t1".to_string());
        let now = Instant::now();
        let mut reg = registry(vec![entry(1, false), entry(2, false)]);

        // 1: retired twice, silent for longer than the threshold — S2b's shape.
        {
            let e = reg.get_mut(&1).unwrap();
            e.suspect_strikes = 2;
            e.last_answered_at = Some(now - SELF_HEAL_SILENT_FOR);
        }
        // 2: just as silent, never missed an answer — an idle connection.
        reg.get_mut(&2).unwrap().last_answered_at = Some(now - SELF_HEAL_SILENT_FOR * 6);
        assert_eq!(routable(&reg, &tap), vec![1, 2], "both are routable before");

        let taken = take_self_heal_candidates(&mut reg, now);
        assert_eq!(
            taken.iter().map(|(id, _, _, _)| *id).collect::<Vec<_>>(),
            vec![1],
            "only the connection that stopped answering is closed"
        );
        assert_eq!(taken[0].3, 2, "the log line reports the strikes");
        assert!(
            reg.get(&1).unwrap().self_healing,
            "the entry has to be marked before the close is sent"
        );
        assert_eq!(
            routable(&reg, &tap),
            vec![2],
            "a connection being closed must not take another request"
        );
    }

    /// An overlapping sweep tick (or a second pass before the close lands) must
    /// not close the same connection twice, and a draining connection is left to
    /// its own departure.
    #[test]
    fn a_connection_is_taken_once_and_a_draining_one_is_left_alone() {
        let now = Instant::now();
        let mut reg = registry(vec![entry(1, false), entry(2, true), entry(3, false)]);
        for id in [1u64, 2, 3] {
            let e = reg.get_mut(&id).unwrap();
            e.suspect_strikes = 1;
            e.last_answered_at = Some(now - SELF_HEAL_SILENT_FOR * 2);
        }

        let first = take_self_heal_candidates(&mut reg, now);
        assert_eq!(
            first.iter().map(|(id, _, _, _)| *id).collect::<Vec<_>>(),
            vec![1, 3],
            "the draining connection (2) leaves on its own terms"
        );
        assert!(
            take_self_heal_candidates(&mut reg, now).is_empty(),
            "a marked connection must not be closed again by the next tick"
        );
    }
}
