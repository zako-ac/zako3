//! Wires the driver to HQ: credentials in, presence and results out.
//!
//! Deliberately thin. It answers "is this tap allowed in" and forwards
//! results; it holds no opinion about caching, permissions or metadata, so
//! that WebSocket termination could later be split into its own service
//! without dragging the audio-request logic with it.

use std::sync::Arc;

use async_trait::async_trait;
use hq_types::hq::TapId as HqTapId;
use hq_types::{OnlineTapState, TapName};
use zakofish4_common::action::DisconnectReason;
use zakofish4_common::event::RequestOutcome;
use zakofish4_common::messages::{TapClientHello, TapServerReject};
use zakofish4_common::model::{HubRejectReasonType, RequestId, TapId};
use zakofish4_hub::{HubBackend, TapHandle};

use super::registry::{Connection, Registry};
use super::GatewayShared;

pub struct GatewayBackend {
    shared: Arc<GatewayShared>,
    /// Allocated per connection, before the tap has even said hello, so every
    /// log line for this socket can be correlated.
    connection_id: u64,
}

impl GatewayBackend {
    pub fn new(shared: Arc<GatewayShared>, connection_id: u64) -> Self {
        Self { shared, connection_id }
    }

    fn registry(&self) -> &Registry {
        &self.shared.registry
    }
}

#[async_trait]
impl HubBackend for GatewayBackend {
    async fn validate(&self, hello: &TapClientHello) -> Result<(), TapServerReject> {
        let tap = match self
            .shared
            .service
            .api_key
            .authenticate_tap(&hello.api_token)
            .await
        {
            Ok(Some(tap)) => tap,
            Ok(None) => {
                return Err(TapServerReject {
                    reason_type: HubRejectReasonType::Unauthorized,
                    reason: "unknown api token".to_string(),
                });
            }
            Err(e) => {
                // An expired key reads as unauthorized; anything else is ours.
                tracing::warn!(%e, "tap authentication failed");
                return Err(TapServerReject {
                    reason_type: HubRejectReasonType::Unauthorized,
                    reason: e.to_string(),
                });
            }
        };

        // The token identifies the tap; the claimed id must match it, or a
        // valid token for tap A could serve requests routed to tap B.
        if tap.id.0 != hello.tap_id.0 {
            tracing::warn!(
                claimed = %hello.tap_id.0,
                actual = %tap.id.0,
                "tap id does not match its api token"
            );
            return Err(TapServerReject {
                reason_type: HubRejectReasonType::Unauthorized,
                reason: "api token does not belong to that tap".to_string(),
            });
        }

        Ok(())
    }

    async fn on_authenticated(&self, hello: &TapClientHello, handle: TapHandle) {
        let hq_tap_id = HqTapId(hello.tap_id.0.clone());

        // Weight comes from the tap's own claim, sanitised. Taking it from the
        // HQ record would be better still; this is the floor.
        let state = OnlineTapState {
            tap_id: hq_tap_id.clone(),
            tap_name: TapName(hello.friendly_name.clone()),
            connection_id: self.connection_id,
            friendly_name: hello.friendly_name.clone(),
            selection_weight: zako3_tap_select::sanitize_weight(hello.selection_weight),
            connected_at: chrono::Utc::now(),
            replica_id: self.shared.replica_id.clone(),
        };

        self.registry()
            .insert(self.connection_id, Connection { state, handle });

        self.shared.publish_presence(&hq_tap_id).await;

        tracing::info!(
            tap_id = %hello.tap_id.0,
            connection_id = self.connection_id,
            replica_id = %self.shared.replica_id,
            resuming = hello.resuming.len(),
            "tap connected"
        );
    }

    async fn on_complete(&self, tap_id: &TapId, request_id: RequestId, outcome: RequestOutcome) {
        let delivered = self.registry().complete(request_id, outcome.clone());

        if !delivered {
            // Expected for the second completion of an audio request: the
            // caller was released when the tap answered, and this is the
            // transfer report arriving afterwards.
            if let RequestOutcome::Streamed(ref s) = outcome {
                tracing::info!(
                    tap_id = %tap_id.0,
                    %request_id,
                    outcome = ?s,
                    "stream outcome reported"
                );
            } else {
                tracing::warn!(
                    tap_id = %tap_id.0,
                    %request_id,
                    "request completed with nobody waiting"
                );
            }
        }
    }

    async fn on_disconnected(&self, tap_id: Option<&TapId>, reason: Option<DisconnectReason>) {
        let removed = self.registry().remove(self.connection_id);

        if let Some(tap_id) = tap_id {
            let hq_tap_id = HqTapId(tap_id.0.clone());
            self.shared.publish_presence(&hq_tap_id).await;

            let uptime_secs = removed
                .as_ref()
                .map(|c| (chrono::Utc::now() - c.state.connected_at).num_seconds().max(0))
                .unwrap_or(0);

            // Instrumented from day one: whether this protocol actually drops
            // connections less often than the one it replaces is the entire
            // question, and it cannot be answered retroactively.
            tracing::info!(
                tap_id = %tap_id.0,
                connection_id = self.connection_id,
                replica_id = %self.shared.replica_id,
                uptime_secs,
                disconnect_reason = reason.map(|r| r.as_str()).unwrap_or("clean"),
                "tap disconnected"
            );
        } else {
            tracing::info!(
                connection_id = self.connection_id,
                disconnect_reason = reason.map(|r| r.as_str()).unwrap_or("clean"),
                "connection closed before authentication"
            );
        }
    }
}
