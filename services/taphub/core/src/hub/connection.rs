use async_trait::async_trait;
use opentelemetry::KeyValue;
use tokio::sync::watch;
use zako3_types::{OnlineTapState, hq::TapId};
use zakofish_taphub::{
    hub::HubHandler,
    types::{HubRejectReasonType, TapClientHello, TapDraining, TapServerReject},
};

use crate::app::App;
use crate::metrics;
use zako3_metrics::TapRedisMetrics;
use zako3_states::TapHubStateService;

use super::{ConnEntry, ConnectionRegistry};

pub struct TapHubConnectionHandler {
    pub(super) app: App,
    pub(super) state_service: TapHubStateService,
    pub(super) metrics_service: TapRedisMetrics,
    pub(super) connections: ConnectionRegistry,
}

impl TapHubConnectionHandler {
    /// Snapshot the current live states for a tap from the authoritative registry.
    ///
    /// Draining connections are left out: the snapshot is what gets published as
    /// "these are online", and a connection on its way out is not.
    fn tap_snapshot(&self, tap_id: &TapId) -> zako3_types::OnlineTapStates {
        let guard = self.connections.lock();
        super::routable_states(&guard, tap_id, std::time::Instant::now())
    }
}

#[async_trait]
impl HubHandler for TapHubConnectionHandler {
    #[tracing::instrument(skip(self, hello), fields(tap_id, connection_id, friendly_name))]
    async fn on_tap_authenticate(
        &self,
        connection_id: u64,
        hello: TapClientHello,
    ) -> Result<(), TapServerReject> {
        tracing::Span::current().record("connection_id", connection_id);
        tracing::Span::current().record("tap_id", &hello.tap_id.0);

        tracing::info!(
            connection_id,
            tap_id = %hello.tap_id.0,
            friendly_name = %hello.friendly_name,
            "Received tap authentication request"
        );

        let tap = if self.app.bypass_hq {
            let hq_tap_id = zako3_types::hq::TapId(hello.tap_id.0.clone());
            Some(crate::handler::tap_lookup::synthetic_tap(&hq_tap_id))
        } else {
            self.app
                .hq_repository
                .authenticate_tap(&hello.api_token)
                .await
        };

        if let Some(tap) = tap {
            if hello.tap_id.0 == tap.id.0 {
                tracing::Span::current().record("tap_id", tracing::field::display(&tap.id.0));
                tracing::Span::current().record(
                    "friendly_name",
                    tracing::field::display(&hello.friendly_name),
                );

                tracing::info!(
                    tap_id = %tap.id.0,
                    connection_id,
                    "Tap authenticated"
                );

                let online_tap = OnlineTapState {
                    tap_id: tap.id.clone(),
                    tap_name: zako3_types::TapName(tap.name.0.clone()),
                    connection_id,
                    friendly_name: hello.friendly_name,
                    selection_weight: hello.selection_weight,
                    connected_at: chrono::Utc::now(),
                    // taphub is a single process and publishes the whole list
                    // itself, so there is no replica to disambiguate.
                    replica_id: String::new(),
                };

                let tap_id = tap.id.clone();
                if let Err(e) = self.metrics_service.register_tap(tap_id).await {
                    tracing::warn!(%e, "Failed to register tap in metrics service");
                }

                // Register in the authoritative in-memory registry, then publish
                // the tap's full live set to Redis with a TTL lease. The registry
                // is the source of truth; Redis is a projection refreshed by the
                // heartbeat in `TapHub::run`.
                let (disconnect_tx, _) = watch::channel(false);
                self.connections.lock().insert(
                    connection_id,
                    ConnEntry {
                        state: online_tap,
                        disconnect_tx,
                        draining: false,
                        unanswered: 0,
                        suspect_until: None,
                        last_answered_at: Some(std::time::Instant::now()),
                        suspect_strikes: 0,
                        inflight_requests: 0,
                    },
                );

                let states = self.tap_snapshot(&tap.id);
                if let Err(e) = self
                    .state_service
                    .publish_tap_states(&tap.id, &states)
                    .await
                {
                    // Roll back the registry insert so we don't advertise a
                    // connection we failed to publish.
                    self.connections.lock().remove(&connection_id);
                    tracing::warn!(%e, "error while publishing tap connection state");
                    return Err(TapServerReject {
                        reason_type: HubRejectReasonType::InternalError,
                        reason: "internal error".to_string(),
                    });
                }

                metrics::metrics().connected_taps.add(1, &[]);
                metrics::metrics()
                    .tap_auth_total
                    .add(1, &[KeyValue::new("result", "ok")]);

                Ok(())
            } else {
                metrics::metrics()
                    .tap_auth_total
                    .add(1, &[KeyValue::new("result", "rejected")]);
                Err(TapServerReject {
                    reason_type: HubRejectReasonType::Unauthorized,
                    reason: "Tap ID mismatch".into(),
                })
            }
        } else {
            let span = tracing::warn_span!("Failed tap authentication", connection_id, friendly_name = %hello.friendly_name);
            let _enter = span.enter();

            metrics::metrics()
                .tap_auth_total
                .add(1, &[KeyValue::new("result", "rejected")]);
            tracing::warn!(
                connection_id,
                "Failed to authenticate tap with provided API token"
            );
            Err(TapServerReject {
                reason_type: HubRejectReasonType::Unauthorized,
                reason: "".into(),
            })
        }
    }

    /// The tap is going away on purpose: stop routing to it and fail what it is
    /// holding, now.
    ///
    /// Without this the connection stays in the routing table until the QUIC
    /// connection gives up on a process that has already stopped reading — which
    /// is what a pod deletion looks like from here, and which is as slow as the
    /// idle timeout. Requests dispatched into that window wait out their whole
    /// budget to learn there was nobody there.
    #[tracing::instrument(skip(self, info), fields(tap_id = %tap_id.0, connection_id))]
    async fn on_tap_draining(&self, tap_id: TapId, connection_id: u64, info: TapDraining) {
        let started_at_unix_ms = info.started_at_unix_ms;

        // Flip the flag and take the disconnect sender in one critical section:
        // after this the connection is not routable and any request currently
        // waiting on this connection is woken with "gone".
        let (already, held) = {
            let mut guard = self.connections.lock();
            match guard.get_mut(&connection_id) {
                Some(entry) if !entry.draining => {
                    entry.draining = true;
                    let held = entry.disconnect_tx.send(true).is_ok();
                    (false, held)
                }
                Some(_) => (true, false),
                // Not (or no longer) ours: a drain notice that arrives after the
                // connection has already been removed has nothing to stop.
                None => return,
            }
        };

        if already {
            tracing::debug!("repeated drain notice; connection was already draining");
            return;
        }

        // The tap's own clock, not ours: this is the whole hop — the notice
        // leaving the process, crossing the network and landing in the routing
        // table — which is the number the scale-down contract is written in.
        let observed_ms = (unix_millis().saturating_sub(started_at_unix_ms)) as f64;

        tracing::warn!(
            tap_id = %tap_id.0,
            connection_id,
            in_flight = info.in_flight,
            wake_delivered = held,
            drain_to_removal_ms = observed_ms as u64,
            "Tap is draining — removed from routing and its in-flight requests failed"
        );

        metrics::metrics()
            .tap_conn_draining_total
            .add(1, &[KeyValue::new("reason", "drain_signal")]);
        metrics::metrics().connected_taps.add(-1, &[]);
        // Milliseconds are the unit the contract is stated in and seconds are
        // what the histogram records; keeping the conversion explicit stops the
        // two from drifting.
        metrics::metrics().tap_conn_drain_to_removal_seconds.record(
            observed_ms / 1000.0,
            &[KeyValue::new("tap_id", tap_id.0.to_string())],
        );

        // Republish without it, so the Redis projection stops advertising a
        // connection that will not take work while the process is still alive.
        let states = self.tap_snapshot(&tap_id);
        if let Err(e) = self
            .state_service
            .publish_tap_states(&tap_id, &states)
            .await
        {
            tracing::warn!(%e, "error while publishing tap connection state on drain");
        }
    }

    #[tracing::instrument(skip(self), fields(tap_id = %tap_id.0, connection_id))]
    async fn on_tap_disconnected(
        &self,
        tap_id: TapId,
        connection_id: u64,
        close_code: Option<u64>,
    ) {
        // Drop the connection from the authoritative registry and fire its
        // disconnect signal so any active streams error out immediately.
        let removed = self.connections.lock().remove(&connection_id);
        let Some(entry) = removed else {
            // Already gone (e.g. failed publish rolled it back) — nothing to do.
            return;
        };

        if entry.draining {
            tracing::info!(
                tap_id = %tap_id.0,
                connection_id,
                close_code = ?close_code,
                "Draining tap finished its connection"
            );
        } else {
            tracing::warn!(
                tap_id = %tap_id.0,
                connection_id,
                "Tap disconnected — erroring all active streams for this connection"
            );
            // Already counted down when the drain was announced; counting twice
            // would make the gauge drift negative over a day of scale-downs.
            metrics::metrics().connected_taps.add(-1, &[]);
        }
        let _ = entry.disconnect_tx.send(true);

        // Why the connection is gone, decided where both halves are known: a
        // drain is the tap saying goodbye, `no_close` is the transport never
        // getting a close frame at all, which is what a process that died or
        // stopped reading looks like from here.
        let reason = match (entry.draining, entry.self_healing, close_code) {
            (true, _, _) => "drain",
            (false, true, _) => "self_heal",
            (false, false, None) => "no_close",
            (false, false, Some(0)) => "clean",
            (false, false, Some(_)) => "close_code",
        };
        metrics::metrics()
            .tap_conn_closed_total
            .add(1, &[KeyValue::new("reason", reason)]);
        tracing::info!(
            tap_id = %tap_id.0,
            connection_id,
            reason,
            close_code = ?close_code,
            inflight_requests = entry.inflight_requests,
            "tap connection closed"
        );

        let uptime_secs = (chrono::Utc::now() - entry.state.connected_at)
            .num_seconds()
            .max(0);
        if let Err(e) = self
            .metrics_service
            .acc_uptime(tap_id.clone(), uptime_secs)
            .await
        {
            tracing::warn!(%e, "Failed to accumulate uptime metric");
        }

        tracing::info!(
            tap_id = %tap_id.0,
            connection_id,
            uptime_secs,
            "Tap disconnected"
        );

        metrics::metrics().connection_duration.record(
            uptime_secs as f64,
            &[KeyValue::new("tap_id", tap_id.0.to_string())],
        );

        // Re-publish the tap's remaining live set (deletes the key if this was the
        // last connection) so the Redis projection reflects the drop right away.
        let states = self.tap_snapshot(&tap_id);
        if let Err(e) = self
            .state_service
            .publish_tap_states(&tap_id, &states)
            .await
        {
            tracing::warn!(%e, "error while publishing tap connection state on disconnect");
        }
    }
}

fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
