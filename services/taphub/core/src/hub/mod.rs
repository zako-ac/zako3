use std::{collections::HashMap, path::Path, sync::Arc, time::Duration};

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
}

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
        let guard = self.connections.lock();

        let available: OnlineTapStates = routable_states(&guard, tap_id);

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
}

/// The connections of one tap that a request may be routed to: live, and not
/// on their way out.
pub(crate) fn routable_states(
    registry: &std::collections::HashMap<u64, ConnEntry>,
    tap_id: &TapId,
) -> OnlineTapStates {
    registry
        .values()
        .filter(|e| &e.state.tap_id == tap_id && !e.draining)
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

    #[test]
    fn a_draining_connection_leaves_the_routing_set_at_once() {
        let tap = TapId("t1".to_string());
        let mut reg = registry(vec![entry(1, false), entry(2, false)]);
        assert_eq!(ids(&routable_states(&reg, &tap)), vec![1, 2]);

        // This is the whole of the routing change: the flag the drain handler
        // flips, read by the same function the request path uses.
        reg.get_mut(&1).unwrap().draining = true;
        assert_eq!(
            ids(&routable_states(&reg, &tap)),
            vec![2],
            "a draining connection must not be selectable"
        );
    }

    #[test]
    fn the_last_tap_draining_leaves_nothing_to_route_to() {
        let tap = TapId("t1".to_string());
        let mut reg = registry(vec![entry(1, false)]);
        reg.get_mut(&1).unwrap().draining = true;
        assert!(routable_states(&reg, &tap).is_empty());
    }

    #[test]
    fn draining_one_tap_does_not_touch_another() {
        let mut other = entry(9, false);
        other.state.tap_id = TapId("t2".to_string());
        let tap = TapId("t1".to_string());
        let reg = registry(vec![entry(1, true), other]);
        assert!(routable_states(&reg, &tap).is_empty());
        assert_eq!(
            ids(&routable_states(&reg, &TapId("t2".to_string()))),
            vec![9]
        );
    }
}
