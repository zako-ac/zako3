//! The tap gateway: `GET /gateway`, upgraded to a WebSocket.
//!
//! Taps used to dial taphub over QUIC and drop constantly. This is a WebSocket
//! over TLS on the ordinary HTTPS port, through the same ingress the web API
//! already uses — which is the actual fix, since the disconnects were the
//! NATs, proxies and firewalls in between rather than anything in the protocol.
//!
//! Only control crosses this connection. Audio goes straight from the tap to a
//! sink over protofish4/UDP, addressed by the `deliver_to` and `encryption_key`
//! carried in each request.
//!
//! Presence answers "is this tap connected"; it does not answer "can this tap
//! still synthesize". A tap whose script engine has wedged keeps its socket,
//! answers heartbeats and refuses every request, and there is nothing in the
//! protocol that says so — so this module also *asks*, on a timer, and keeps
//! the answer in [`TapHealthService`] where routing and operators can see it.

pub mod backend;
pub mod dispatch;
pub mod dispatcher;
pub mod registry;
pub mod transport;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use hq_core::Service;
use zako3_states::{
    ConnectionHealth, ConnectionVerdict, GatewayPresenceService, TapHealthService, TapHealthView,
};
use hq_types::hq::TapId as HqTapId;
use hq_types::{OnlineTapState, OnlineTapStates};
use zakofish4_common::config::HubConfig;
use zakofish4_common::messages::{ProbeResult, ResponseVariant};
use zakofish4_common::state::PendingRequest;
use zakofish4_tap_select_reexport::DynamicSampler;

use backend::GatewayBackend;
use dispatch::{DispatchError, dispatch_local, dispatch_remote};
use registry::Registry;
use transport::AxumTransport;

/// Re-export so callers do not need a direct dependency for one type.
pub mod zakofish4_tap_select_reexport {
    pub use zako3_tap_select::DynamicSampler;
}

/// How often presence leases are refreshed, as a fraction of the TTL.
///
/// A third, so two consecutive refreshes can fail before a healthy replica
/// looks dead.
const HEARTBEAT_DIVISOR: u32 = 3;

/// How many connections one request may be tried against before giving up.
///
/// Three rather than two because the reasons to move on can stack: a stale
/// presence entry, a refusal that asked for someone else, and a connection
/// health has already ruled out. Each attempt costs at most one dispatch
/// timeout, so this is also a bound on how long a request spends failing.
const MAX_DISPATCH_ATTEMPTS: usize = 3;

/// The protocol version from which a tap can be probed.
///
/// The probe is the only thing the gateway asks that a tap might not
/// understand, and an unanswerable question must not be read as a bad answer.
const PROBE_MIN_PROTOCOL_VERSION: u32 = 2;

/// How the gateway proves a tap can still synthesize.
#[derive(Debug, Clone)]
pub struct ProbeConfig {
    /// Gap between probe rounds for a tap.
    pub interval: Duration,
    /// How long the gateway waits for an answer before giving up. Kept above
    /// the hub's own `probe_timeout` so the specific verdict wins the race.
    pub timeout: Duration,
    /// A first sample slower than this makes a connection `Slow` rather than
    /// `Healthy`: deprioritised, but still perfectly routable.
    pub slow_first_sample_ms: u64,
}

impl Default for ProbeConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(20),
            slow_first_sample_ms: zako3_states::DEFAULT_SLOW_FIRST_SAMPLE_MS,
        }
    }
}

/// Shared across every connection this replica holds.
pub struct GatewayShared {
    pub service: Service,
    pub registry: Registry,
    pub presence: GatewayPresenceService,
    /// What probing has concluded, per replica, in the same Redis.
    pub health: TapHealthService,
    pub nats: Option<async_nats::Client>,
    /// Identifies this process in presence entries and NATS subjects. In
    /// Kubernetes the pod name; anything stable and unique will do.
    pub replica_id: String,
    pub hub_config: HubConfig,
    pub probe: ProbeConfig,
    /// Probe ids handed out by this replica.
    ///
    /// A plain counter is enough because a probe never leaves the process: the
    /// connection it asks about is one this replica is holding, so the id only
    /// has to be unique among the probes this replica is waiting on.
    probe_seq: AtomicU64,
}

impl GatewayShared {
    /// Republish this replica's connections for a tap.
    ///
    /// Always the full list *for this replica*, never a delta: the replica is
    /// the only writer of its own key, so a whole-list write cannot race with
    /// itself and needs no read-modify-write.
    pub async fn publish_presence(&self, tap_id: &HqTapId) {
        let states = self.registry.states_for(tap_id);
        if let Err(e) = self
            .presence
            .publish(tap_id, &self.replica_id, &states)
            .await
        {
            tracing::warn!(%e, tap_id = %tap_id.0, "failed to publish gateway presence");
        }
    }

    fn next_probe_id(&self) -> u64 {
        self.probe_seq.fetch_add(1, Ordering::SeqCst)
    }
}

/// The gateway, as HQ's other code sees it.
#[derive(Clone)]
pub struct Gateway {
    shared: Arc<GatewayShared>,
}

impl Gateway {
    pub fn new(
        service: Service,
        presence: GatewayPresenceService,
        health: TapHealthService,
        nats: Option<async_nats::Client>,
        replica_id: String,
    ) -> Self {
        let shared = Arc::new(GatewayShared {
            service,
            registry: Registry::new(),
            presence,
            health,
            nats,
            replica_id,
            hub_config: HubConfig::default(),
            probe: ProbeConfig::default(),
            probe_seq: AtomicU64::new(1),
        });

        spawn_lease_heartbeat(Arc::clone(&shared));
        spawn_probe_loop(Arc::clone(&shared));
        Self { shared }
    }

    pub fn shared(&self) -> &Arc<GatewayShared> {
        &self.shared
    }

    pub fn replica_id(&self) -> &str {
        &self.shared.replica_id
    }

    /// Connections this replica is holding, for `/metrics` and admin views.
    pub fn local_connection_count(&self) -> usize {
        self.shared.registry.len()
    }

    /// The health store, for the audio service to report first samples into.
    pub fn health_service(&self) -> TapHealthService {
        self.shared.health.clone()
    }

    /// Everything known about a tap's health, merged across replicas.
    pub async fn health(&self, tap_id: &HqTapId) -> TapHealthView {
        self.shared.health.get(tap_id).await.unwrap_or_default()
    }

    /// Whether this tap is worth routing to at all.
    ///
    /// False only when every probed connection has failed. A tap that has never
    /// been probed — most of them — is usable, which is what keeps this from
    /// being a second, stricter definition of "online".
    pub async fn is_usable(&self, tap_id: &HqTapId) -> bool {
        self.health(tap_id).await.is_usable()
    }

    /// Connections a router must not pick, as `(replica, connection)`.
    pub async fn excluded_connections(&self, tap_id: &HqTapId) -> Vec<(String, u64)> {
        self.health(tap_id).await.excluded_connections()
    }

    /// Send a request to one of a tap's connections and wait for its answer.
    ///
    /// The connection is picked by weight across every replica, then the
    /// request is routed locally or over NATS. A connection is abandoned and
    /// another tried when it looks stale, when health has already ruled it out,
    /// or when the tap itself answered "ask someone else" — but never merely
    /// because it was slow: each attempt costs a full dispatch timeout, and the
    /// audio engine's budget is sized for one.
    ///
    /// The `request_id` is deliberately the same on every attempt. The sink's
    /// armed receiver and `publish_route` are both keyed by it and are not
    /// reachable from here, so a new one could not be handed out; and every
    /// reason this loop retries is a reason the previous connection provably
    /// never streamed under it. A connection that has started streaming is
    /// never abandoned — that is what stops a hop from producing two streams
    /// for one request.
    pub async fn request(
        &self,
        tap_id: &HqTapId,
        make_pending: impl Fn() -> PendingRequest,
    ) -> Result<ResponseVariant, DispatchError> {
        let states = self.shared.presence.get(tap_id).await.unwrap_or_default();
        if states.is_empty() {
            return Err(DispatchError::NoConnection(tap_id.0.clone()));
        }

        let health = self.shared.health.get(tap_id).await.unwrap_or_default();
        if !health.is_usable() {
            tracing::warn!(
                tap_id = %tap_id.0,
                connections = states.len(),
                "every probed connection of this tap is unhealthy"
            );
            return Err(DispatchError::NoConnection(tap_id.0.clone()));
        }

        let mut picker = CandidatePicker::new(health);
        for attempt in 0..MAX_DISPATCH_ATTEMPTS {
            let Some(picked) = picker.next(&states) else {
                break;
            };

            let result = if picked.replica_id == self.shared.replica_id {
                dispatch_local(&self.shared, picked.connection_id, make_pending()).await
            } else {
                dispatch_remote(
                    &self.shared,
                    &picked.replica_id,
                    picked.connection_id,
                    make_pending(),
                )
                .await
            };

            match result {
                Err(DispatchError::NoConnection(_)) | Err(DispatchError::Disconnected) => {
                    tracing::debug!(
                        tap_id = %tap_id.0,
                        connection_id = picked.connection_id,
                        replica_id = %picked.replica_id,
                        "stale connection; retrying with another"
                    );
                    picker.rule_out(&picked);
                }
                // The tap answered, and its answer was "not me, try another".
                // Nothing was streamed, so the request id is still unused.
                Ok(ResponseVariant::AudioRequestFailure(ref f)) if f.try_others => {
                    tracing::debug!(
                        tap_id = %tap_id.0,
                        connection_id = picked.connection_id,
                        reason = %f.reason,
                        "tap asked for another connection; retrying"
                    );
                    picker.rule_out(&picked);
                }
                other => return other,
            }

            tracing::debug!(attempt, tap_id = %tap_id.0, "dispatch attempt failed");
        }

        Err(DispatchError::NoConnection(tap_id.0.clone()))
    }

    /// Tell a tap to stop streaming a request.
    pub async fn cancel(&self, tap_id: &HqTapId, request_id: zakofish4_common::model::RequestId) {
        dispatch::cancel(&self.shared, tap_id, request_id).await;
    }
}

/// Which connection to try next for one request.
///
/// Split out of [`Gateway::request`] so the routing decision — skip what health
/// has ruled out, deprioritise what is busy, never pick the same connection
/// twice — can be exercised without Redis, a socket or an audio request.
struct CandidatePicker {
    health: TapHealthView,
    /// Connections this request has already been through. Keyed by replica as
    /// well as id: a connection id is allocated per process, so replica A's
    /// connection 3 and replica B's connection 3 are different connections.
    tried: Vec<(String, u64)>,
    sampler: DynamicSampler,
}

impl CandidatePicker {
    fn new(health: TapHealthView) -> Self {
        Self {
            health,
            tried: Vec::new(),
            sampler: DynamicSampler::new(),
        }
    }

    /// The next connection to try, or `None` when none is left.
    fn next(&mut self, states: &OnlineTapStates) -> Option<OnlineTapState> {
        let Self {
            health,
            tried,
            sampler,
        } = self;

        sampler
            .next_state_with(states, |s| {
                if tried
                    .iter()
                    .any(|(r, c)| r == &s.replica_id && *c == s.connection_id)
                {
                    return None;
                }
                if health.is_excluded(&s.replica_id, s.connection_id) {
                    return None;
                }
                Some(health.weight_scale(&s.replica_id, s.connection_id))
            })
            .cloned()
    }

    /// Never pick this connection again for this request.
    fn rule_out(&mut self, state: &OnlineTapState) {
        if !self
            .tried
            .iter()
            .any(|(r, c)| r == &state.replica_id && *c == state.connection_id)
        {
            self.tried
                .push((state.replica_id.clone(), state.connection_id));
        }
    }
}

/// `GET /gateway` — upgrade and hand the socket to the driver.
pub async fn gateway_handler(
    State(gateway): State<Gateway>,
    ws: WebSocketUpgrade,
) -> Response {
    let shared = Arc::clone(&gateway.shared);
    let connection_id = shared.registry.next_connection_id();

    ws.on_upgrade(move |socket| async move {
        // Subscribing before the driver runs means a dispatch cannot arrive for
        // a connection this replica is not yet listening for.
        dispatch::serve_subject(Arc::clone(&shared), connection_id);

        let backend = Arc::new(GatewayBackend::new(Arc::clone(&shared), connection_id));
        let cfg = shared.hub_config.clone();
        zakofish4_hub::serve(AxumTransport::new(socket), backend, cfg).await;
    })
}

/// Keep this replica's presence leases from lapsing while its connections live.
fn spawn_lease_heartbeat(shared: Arc<GatewayShared>) {
    let ttl = shared.presence.lease_ttl_secs();
    let interval = Duration::from_secs((ttl / HEARTBEAT_DIVISOR as u64).max(1));

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            for tap_id in shared.registry.tap_ids() {
                shared.publish_presence(&tap_id).await;
            }
        }
    });
}

/// Ask every tap this replica holds whether it can still synthesize.
///
/// Only this replica's own connections are probed, never another replica's: a
/// probe runs where the connection lives, and reaching across processes for it
/// would buy nothing but a second way to be wrong. Each replica's verdict is
/// published under its own key and readers union them, so the answer a router
/// acts on does not depend on which replica it asked.
fn spawn_probe_loop(shared: Arc<GatewayShared>) {
    let interval = shared.probe.interval.max(Duration::from_millis(1));

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            for tap_id in shared.registry.tap_ids() {
                probe_tap(&shared, &tap_id).await;
            }
        }
    });
}

/// Probe every probeable connection this replica holds for one tap, and publish
/// what came back.
async fn probe_tap(shared: &Arc<GatewayShared>, tap_id: &HqTapId) {
    let states = shared.registry.states_for(tap_id);

    let mut results = Vec::with_capacity(states.len());
    for state in &states {
        // A tap too old to answer would look exactly like a tap that is wedged,
        // so it is not asked. Leaving it out of the list leaves it `Unknown`,
        // which routes exactly as it did before any of this existed.
        if !shared.registry.probe_supported(state.connection_id) {
            continue;
        }
        results.push(probe_connection(shared, state).await);
    }

    // Nothing probed — every connection gone, or every tap too old to answer.
    // Publishing an empty list withdraws this replica's opinion instead of
    // recording a verdict nobody measured.
    if let Err(e) = shared
        .health
        .publish_probe(tap_id, &shared.replica_id, &results)
        .await
    {
        tracing::warn!(%e, tap_id = %tap_id.0, "failed to publish tap health");
    }
}

/// Ask one connection to prove itself and turn the answer into a verdict.
async fn probe_connection(
    shared: &Arc<GatewayShared>,
    state: &OnlineTapState,
) -> ConnectionHealth {
    let connection_id = state.connection_id;
    let probed_at = chrono::Utc::now();

    let Some(conn) = shared.registry.get(connection_id) else {
        return failed(connection_id, "not held by this replica", probed_at);
    };

    let probe_id = shared.next_probe_id();
    let rx = shared.registry.await_probe(probe_id);

    if !conn.handle.probe(probe_id) {
        shared.registry.forget_probe(probe_id);
        return failed(connection_id, "connection closed before the probe went out", probed_at);
    }

    // Headroom over the hub's own deadline, so a real verdict wins over this
    // backstop rather than racing it.
    match tokio::time::timeout(shared.probe.timeout + Duration::from_secs(2), rx).await {
        Ok(Ok(Some(ProbeResult::Ready {
            time_to_first_sample_ms,
        }))) => {
            let verdict = if time_to_first_sample_ms > shared.probe.slow_first_sample_ms {
                ConnectionVerdict::Slow
            } else {
                ConnectionVerdict::Healthy
            };
            ConnectionHealth {
                connection_id,
                verdict,
                time_to_first_sample_ms: Some(time_to_first_sample_ms),
                reason: None,
                probed_at,
            }
        }
        Ok(Ok(Some(ProbeResult::Failed { reason }))) => {
            ConnectionHealth {
                connection_id,
                verdict: ConnectionVerdict::Failed,
                time_to_first_sample_ms: None,
                reason: Some(reason),
                probed_at,
            }
        }
        // The tap has no opinion, which is not the same as a bad one: it is
        // what every tap that has not implemented the probe answers.
        Ok(Ok(Some(ProbeResult::Unsupported))) => ConnectionHealth {
            connection_id,
            verdict: ConnectionVerdict::Unsupported,
            time_to_first_sample_ms: None,
            reason: None,
            probed_at,
        },
        Ok(Ok(None)) | Ok(Err(_)) => {
            failed(connection_id, "no answer within the probe deadline", probed_at)
        }
        Err(_) => {
            shared.registry.forget_probe(probe_id);
            failed(connection_id, "no answer within the probe deadline", probed_at)
        }
    }
}

fn failed(connection_id: u64, reason: &str, probed_at: chrono::DateTime<chrono::Utc>) -> ConnectionHealth {
    ConnectionHealth {
        connection_id,
        verdict: ConnectionVerdict::Failed,
        time_to_first_sample_ms: None,
        reason: Some(reason.to_string()),
        probed_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use hq_types::TapName;
    use zako3_states::{
        ConnectionHealth, ConnectionVerdict, TapHealth, TapHealthView, TapLatency,
    };

    fn conn(replica: &str, connection_id: u64, weight: f32) -> OnlineTapState {
        OnlineTapState {
            tap_id: HqTapId("tap-1".into()),
            tap_name: TapName("test".into()),
            connection_id,
            friendly_name: format!("c{connection_id}"),
            selection_weight: weight,
            connected_at: Utc::now(),
            replica_id: replica.into(),
        }
    }

    fn verdict(connection_id: u64, v: ConnectionVerdict) -> ConnectionHealth {
        ConnectionHealth {
            connection_id,
            verdict: v,
            time_to_first_sample_ms: None,
            reason: None,
            probed_at: Utc::now(),
        }
    }

    fn health(replica: &str, connections: Vec<ConnectionHealth>) -> TapHealthView {
        TapHealthView::from_records(
            vec![TapHealth {
                replica_id: replica.into(),
                connections,
                consecutive_failures: 0,
                last_probe_at: Some(Utc::now()),
                last_probe_result: None,
                time_to_first_sample_ms: None,
                updated_at: Utc::now(),
            }],
            None,
        )
    }

    /// The hop that matters: the first connection has already been judged
    /// unable to synthesize, so the request must land on the other one — not
    /// be retried against the same connection and not fail outright.
    #[test]
    fn a_hop_skips_a_connection_health_has_failed() {
        let states = vec![conn("r1", 1, 1.0), conn("r1", 2, 1.0)];
        let health = health("r1", vec![verdict(1, ConnectionVerdict::Failed)]);

        let mut picker = CandidatePicker::new(health);
        for _ in 0..20 {
            let picked = picker.next(&states).expect("the healthy connection");
            assert_eq!(
                picked.connection_id, 2,
                "a failed connection must never be picked"
            );
        }
    }

    /// The same hop, one step earlier: the connection was fine, the request
    /// failed on it, and the retry must not go back to it.
    #[test]
    fn a_hop_never_repeats_a_connection_it_already_tried() {
        let states = vec![conn("r1", 1, 1.0), conn("r1", 2, 1.0)];
        let mut picker = CandidatePicker::new(TapHealthView::default());

        let first = picker.next(&states).unwrap();
        picker.rule_out(&first);

        for _ in 0..20 {
            let picked = picker.next(&states).expect("the other connection");
            assert_ne!(
                picked.connection_id, first.connection_id,
                "a retry must move on, not repeat"
            );
        }
    }

    /// A connection id is only unique within a replica, so an exclusion that
    /// matched on the id alone would rule out an unrelated connection that
    /// happened to be allocated the same number elsewhere.
    #[test]
    fn exclusion_is_scoped_to_the_replica_that_probed() {
        let states = vec![conn("r1", 7, 1.0), conn("r2", 7, 1.0)];
        let health = health("r1", vec![verdict(7, ConnectionVerdict::Failed)]);

        let mut picker = CandidatePicker::new(health);
        for _ in 0..20 {
            let picked = picker.next(&states).expect("r2's connection 7");
            assert_eq!(picked.replica_id, "r2");
        }
    }

    /// A busy tap is deprioritised, never hidden. It is the only tap that can
    /// serve its own id, so making it unreachable would turn "slow" into
    /// "broken" for every listener.
    #[test]
    fn a_busy_tap_with_one_connection_is_still_reachable() {
        let states = vec![conn("r1", 1, 1.0)];
        let busy = TapHealthView::from_records(
            vec![],
            Some(TapLatency {
                time_to_first_sample_ms: Some(9_000),
                consecutive_slow: 4,
                busy_until: Some(Utc::now() + chrono::Duration::seconds(30)),
                reason: Some("slow".into()),
                updated_at: Utc::now(),
            }),
        );

        assert!(busy.is_busy());
        assert!(busy.is_usable(), "busy is not unhealthy");

        let mut picker = CandidatePicker::new(busy);
        assert!(
            picker.next(&states).is_some(),
            "a busy tap must still be picked when it is all there is"
        );
    }

    /// And with somewhere better to go, a busy tap is the second choice.
    #[test]
    fn a_slow_connection_loses_to_a_healthy_one() {
        let states = vec![conn("r1", 1, 1.0), conn("r1", 2, 1.0)];
        let health = health("r1", vec![verdict(1, ConnectionVerdict::Slow)]);

        let mut picker = CandidatePicker::new(health);
        for _ in 0..20 {
            let picked = picker.next(&states).unwrap();
            assert_eq!(
                picked.connection_id, 2,
                "a slow connection must be deprioritised behind a healthy one"
            );
        }
    }

    /// A tap nobody has probed must route exactly as it did before the health
    /// store existed — otherwise shipping this would take every tap offline.
    #[test]
    fn an_unprobed_tap_is_usable() {
        let view = TapHealthView::default();
        assert_eq!(view.verdict(), zako3_states::TapVerdict::Unknown);
        assert!(view.is_usable());
        assert!(view.excluded_connections().is_empty());
    }

    /// The other side of that: evidence that every connection is broken must
    /// actually stop a request, or the store is decoration.
    #[test]
    fn a_tap_whose_every_connection_failed_is_not_usable() {
        let health = health(
            "r1",
            vec![
                verdict(1, ConnectionVerdict::Failed),
                verdict(2, ConnectionVerdict::Failed),
            ],
        );
        assert!(!health.is_usable());
        assert_eq!(health.excluded_connections().len(), 2);
    }

    /// A tap that has told us nothing about itself must keep being routed to.
    #[test]
    fn an_unsupported_probe_is_not_a_failure() {
        let health = health("r1", vec![verdict(1, ConnectionVerdict::Unsupported)]);
        assert!(health.is_usable());
        assert!(!health.is_excluded("r1", 1));

        let states = vec![conn("r1", 1, 1.0)];
        let mut picker = CandidatePicker::new(health);
        assert!(picker.next(&states).is_some());
    }

    /// With every candidate ruled out there is nothing left to try, and the
    /// caller must be told rather than handed a connection it just failed on.
    #[test]
    fn an_exhausted_picker_yields_nothing() {
        let states = vec![conn("r1", 1, 1.0)];
        let mut picker = CandidatePicker::new(TapHealthView::default());

        let only = picker.next(&states).unwrap();
        picker.rule_out(&only);
        assert!(picker.next(&states).is_none());
    }
}
