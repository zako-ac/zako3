use async_trait::async_trait;
use tokio::sync::watch;
use zakofish::{
    hub::HubHandler,
    types::{HubRejectReasonType, TapClientHello, TapServerReject},
};
use zako3_types::{OnlineTapState, hq::TapId};

use crate::app::App;
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
    async fn on_tap_authenticate(
        &self,
        connection_id: u64,
        hello: TapClientHello,
    ) -> Result<(), TapServerReject> {
        let tap = self
            .app
            .hq_repository
            .authenticate_tap(&hello.api_token)
            .await;

        if let Some(tap) = tap {
            if hello.tap_id == tap.id {
                tracing::info!(
                    "Hub: Tap authenticated! ID: {:?}, Name: {}, Connection: {}",
                    tap.id,
                    tap.name.0,
                    connection_id
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

                Ok(())
            } else {
                Err(TapServerReject {
                    reason_type: HubRejectReasonType::Unauthorized,
                    reason: "Tap ID mismatch".into(),
                })
            }
        } else {
            Err(TapServerReject {
                reason_type: HubRejectReasonType::Unauthorized,
                reason: "".into(),
            })
        }
    }

    async fn on_tap_disconnected(&self, tap_id: TapId, connection_id: u64) {
        tracing::info!(
            "Hub: Tap disconnected! ID: {:?}, Connection: {}",
            tap_id,
            connection_id
        );

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
        if let Some(conn) = states.iter().find(|s| s.connection_id == connection_id) {
            let secs = (chrono::Utc::now() - conn.connected_at)
                .num_seconds()
                .max(0);
            if let Err(e) = self.metrics_service.acc_uptime(tap_id.clone(), secs).await {
                tracing::warn!(%e, "Failed to accumulate uptime metric");
            }
        }

        if let Err(e) = self
            .state_service
            .remove_connection_state(&tap_id, connection_id)
            .await
        {
            tracing::warn!(%e, "error while setting tap connection state");
        }
    }
}
