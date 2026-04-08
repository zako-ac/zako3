use async_trait::async_trait;
use opentelemetry::KeyValue;
use tokio::sync::watch;
use zakofish::{
    hub::HubHandler,
    types::{HubRejectReasonType, TapClientHello, TapServerReject},
};
use zako3_types::{OnlineTapState, hq::TapId};

use crate::app::App;
use crate::metrics;
use zako3_states::{TapHubStateService, TapMetricsStateService};

use super::ConnectionSignals;

pub struct TapHubConnectionHandler {
    pub(super) app: App,
    pub(super) state_service: TapHubStateService,
    pub(super) metrics_service: TapMetricsStateService,
    pub(super) connection_signals: ConnectionSignals,
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

        let tap = self
            .app
            .hq_repository
            .authenticate_tap(&hello.api_token)
            .await;

        if let Some(tap) = tap {
            if hello.tap_id == tap.id {
                tracing::Span::current().record("tap_id", tracing::field::display(&tap.id.0));
                tracing::Span::current()
                    .record("friendly_name", tracing::field::display(&hello.friendly_name));

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

                self.state_service
                    .set_connection_state(online_tap)
                    .await
                    .map_err(|e| {
                        tracing::warn!(%e, "error while setting tap connection state");
                        TapServerReject {
                            reason_type: HubRejectReasonType::InternalError,
                            reason: "internal error".to_string(),
                        }
                    })?;

                let (disconnect_tx, _) = watch::channel(false);
                self.connection_signals.lock().insert(connection_id, disconnect_tx);

                metrics::metrics().connected_taps.add(1, &[]);
                metrics::metrics().tap_auth_total.add(
                    1,
                    &[KeyValue::new("result", "ok")],
                );

                Ok(())
            } else {
                metrics::metrics().tap_auth_total.add(
                    1,
                    &[KeyValue::new("result", "rejected")],
                );
                Err(TapServerReject {
                    reason_type: HubRejectReasonType::Unauthorized,
                    reason: "Tap ID mismatch".into(),
                })
            }
        } else {
            metrics::metrics().tap_auth_total.add(
                1,
                &[KeyValue::new("result", "rejected")],
            );
            Err(TapServerReject {
                reason_type: HubRejectReasonType::Unauthorized,
                reason: "".into(),
            })
        }
    }

    #[tracing::instrument(skip(self), fields(tap_id = %tap_id.0, connection_id))]
    async fn on_tap_disconnected(&self, tap_id: TapId, connection_id: u64) {
        if let Some(tx) = self.connection_signals.lock().remove(&connection_id) {
            tracing::warn!(
                tap_id = %tap_id.0,
                connection_id,
                "Tap disconnected — erroring all active streams for this connection"
            );
            let _ = tx.send(true);
        }

        let states = self
            .state_service
            .get_tap_states(&tap_id)
            .await
            .unwrap_or_default();

        let uptime_secs = if let Some(conn) = states.iter().find(|s| s.connection_id == connection_id) {
            let secs = (chrono::Utc::now() - conn.connected_at)
                .num_seconds()
                .max(0);
            if let Err(e) = self.metrics_service.acc_uptime(tap_id.clone(), secs).await {
                tracing::warn!(%e, "Failed to accumulate uptime metric");
            }
            secs
        } else {
            0
        };

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

        if let Err(e) = self
            .state_service
            .remove_connection_state(&tap_id, connection_id)
            .await
        {
            tracing::warn!(%e, "error while removing tap connection state");
        }
    }
}
