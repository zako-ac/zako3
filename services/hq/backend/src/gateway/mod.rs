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

pub mod backend;
pub mod dispatch;
pub mod dispatcher;
pub mod registry;
pub mod transport;

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use hq_core::Service;
use zako3_states::GatewayPresenceService;
use hq_types::hq::TapId as HqTapId;
use zakofish4_common::config::HubConfig;
use zakofish4_common::messages::ResponseVariant;
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

/// Shared across every connection this replica holds.
pub struct GatewayShared {
    pub service: Service,
    pub registry: Registry,
    pub presence: GatewayPresenceService,
    pub nats: Option<async_nats::Client>,
    /// Identifies this process in presence entries and NATS subjects. In
    /// Kubernetes the pod name; anything stable and unique will do.
    pub replica_id: String,
    pub hub_config: HubConfig,
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
        nats: Option<async_nats::Client>,
        replica_id: String,
    ) -> Self {
        let shared = Arc::new(GatewayShared {
            service,
            registry: Registry::new(),
            presence,
            nats,
            replica_id,
            hub_config: HubConfig::default(),
        });

        spawn_lease_heartbeat(Arc::clone(&shared));
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

    /// Send a request to one of a tap's connections and wait for its answer.
    ///
    /// Picks a connection by weight across every replica, then routes locally
    /// or over NATS. On a stale presence entry it retries once with a different
    /// connection — a tap that reconnected to another replica leaves a warm
    /// lease behind, and the first pick may land on it.
    pub async fn request(
        &self,
        tap_id: &HqTapId,
        make_pending: impl Fn() -> PendingRequest,
    ) -> Result<ResponseVariant, DispatchError> {
        let mut states = self.shared.presence.get(tap_id).await.unwrap_or_default();
        if states.is_empty() {
            return Err(DispatchError::NoConnection(tap_id.0.clone()));
        }

        let mut sampler = DynamicSampler::new();
        for attempt in 0..2 {
            let Some(picked) = sampler.next_state(&states).cloned() else {
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
                Err(DispatchError::NoConnection(_)) | Err(DispatchError::Disconnected)
                    if attempt == 0 =>
                {
                    tracing::debug!(
                        tap_id = %tap_id.0,
                        connection_id = picked.connection_id,
                        replica_id = %picked.replica_id,
                        "stale connection; retrying with another"
                    );
                    states.retain(|s| s.connection_id != picked.connection_id);
                    if states.is_empty() {
                        return Err(DispatchError::NoConnection(tap_id.0.clone()));
                    }
                }
                other => return other,
            }
        }

        Err(DispatchError::NoConnection(tap_id.0.clone()))
    }

    /// Tell a tap to stop streaming a request.
    pub async fn cancel(&self, tap_id: &HqTapId, request_id: zakofish4_common::model::RequestId) {
        dispatch::cancel(&self.shared, tap_id, request_id).await;
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
