use async_trait::async_trait;
use opentelemetry::KeyValue;
use tokio::sync::watch;
use zako3_types::{OnlineTapState, hq::TapId};
use zakofish_taphub::{
    hub::HubHandler,
    types::{HubRejectReasonType, TapClientHello, TapServerReject},
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
    fn tap_snapshot(&self, tap_id: &TapId) -> zako3_types::OnlineTapStates {
        self.connections
            .lock()
            .values()
            .filter(|e| &e.state.tap_id == tap_id)
            .map(|e| e.state.clone())
            .collect()
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
                    },
                );

                let states = self.tap_snapshot(&tap.id);
                if let Err(e) = self.state_service.publish_tap_states(&tap.id, &states).await {
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

    #[tracing::instrument(skip(self), fields(tap_id = %tap_id.0, connection_id))]
    async fn on_tap_disconnected(&self, tap_id: TapId, connection_id: u64) {
        // Drop the connection from the authoritative registry and fire its
        // disconnect signal so any active streams error out immediately.
        let removed = self.connections.lock().remove(&connection_id);
        let Some(entry) = removed else {
            // Already gone (e.g. failed publish rolled it back) — nothing to do.
            return;
        };

        tracing::warn!(
            tap_id = %tap_id.0,
            connection_id,
            "Tap disconnected — erroring all active streams for this connection"
        );
        let _ = entry.disconnect_tx.send(true);

        let uptime_secs = (chrono::Utc::now() - entry.state.connected_at)
            .num_seconds()
            .max(0);
        if let Err(e) = self.metrics_service.acc_uptime(tap_id.clone(), uptime_secs).await {
            tracing::warn!(%e, "Failed to accumulate uptime metric");
        }

        tracing::info!(
            tap_id = %tap_id.0,
            connection_id,
            uptime_secs,
            "Tap disconnected"
        );

        metrics::metrics().connected_taps.add(-1, &[]);
        metrics::metrics().connection_duration.record(
            uptime_secs as f64,
            &[KeyValue::new("tap_id", tap_id.0.to_string())],
        );

        // Re-publish the tap's remaining live set (deletes the key if this was the
        // last connection) so the Redis projection reflects the drop right away.
        let states = self.tap_snapshot(&tap_id);
        if let Err(e) = self.state_service.publish_tap_states(&tap_id, &states).await {
            tracing::warn!(%e, "error while publishing tap connection state on disconnect");
        }
    }
}
