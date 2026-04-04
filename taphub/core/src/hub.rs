use async_trait::async_trait;
use protofish2::connection::ServerConfig;
use zako3_types::{OnlineTapState, hq::TapId};
use zakofish::{
    hub::{HubHandler, ZakofishHub},
    types::{HubRejectReasonType, TapClientHello, TapServerReject},
};

use crate::{app::App, service::state::StateService};

pub struct TapHubConnectionHandler {
    app: App,
    state_service: StateService,
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
                    "Hub: Tap authenticated! ID: {:?}, Connection: {}",
                    tap.id,
                    connection_id
                );

                let online_tap = OnlineTapState {
                    tap_id: tap.id.clone(),
                    connection_id,
                    friendly_name: hello.friendly_name,
                    selection_weight: hello.selection_weight,
                };
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
    zf_hub: ZakofishHub,
}

impl TapHub {
    pub fn new(app: App) -> Self {}
}
