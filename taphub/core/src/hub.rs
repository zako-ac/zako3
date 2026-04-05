use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use parking_lot::Mutex;
use zako3_types::{OnlineTapState, hq::TapId};
use zakofish::{
    ZakofishError, create_server_config,
    hub::{HubHandler, ZakofishHub},
    types::{HubRejectReasonType, TapClientHello, TapServerReject},
};

use crate::{app::App, routing::DynamicSampler};
use zako3_states::{TapHubStateService, TapMetricsStateService};

pub struct TapHubConnectionHandler {
    app: App,
    state_service: TapHubStateService,
    metrics_service: TapMetricsStateService,
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
                };

                let tap_id = tap.id.clone();
                if let Err(e) = self.metrics_service.register_tap(tap_id.clone()).await {
                    tracing::warn!(%e, "Failed to register tap in metrics service");
                }
                if let Err(e) = self.metrics_service.inc_active_now(tap_id).await {
                    tracing::warn!(%e, "Failed to increment active_now metric");
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

        if let Err(e) = self.metrics_service.dec_active_now(tap_id.clone()).await {
            tracing::warn!(%e, "Failed to decrement active_now metric");
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

pub struct TapHub {
    pub zf_hub: ZakofishHub,
    pub sampler: Arc<Mutex<DynamicSampler>>,
    pub state_service: TapHubStateService,
    pub metrics_service: TapMetricsStateService,
    pub app: App,
}

impl TapHub {
    pub fn new(
        app: App,
        bind_address: &str,
        cert_file: impl AsRef<Path>,
        key_file: impl AsRef<Path>,
    ) -> Result<Self, ZakofishError> {
        let server_config = create_server_config(
            bind_address.parse().map_err(|_| {
                ZakofishError::ProtocolError("Invalid bind address is provided".to_string())
            })?,
            cert_file,
            key_file,
        )?;

        let handler = TapHubConnectionHandler {
            app: app.clone(),
            state_service: app.tap_state_service.clone(),
            metrics_service: app.tap_metrics_service.clone(),
        };

        let zf_hub = ZakofishHub::new(server_config, Arc::new(handler))?;

        Ok(Self {
            zf_hub,
            sampler: Arc::new(Mutex::new(DynamicSampler::new())),
            state_service: app.tap_state_service.clone(),
            metrics_service: app.tap_metrics_service.clone(),
            app,
        })
    }

    pub async fn run(&self) -> Result<(), ZakofishError> {
        self.zf_hub.run().await
    }
}
