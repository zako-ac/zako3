//! Connects the audio service to the gateway.
//!
//! `hq-core` knows only the [`TapDispatcher`] trait, so the audio-request logic
//! carries no dependency on WebSockets, Redis presence or NATS — and stays
//! testable without any of them.

use async_trait::async_trait;
use hq_core::service::audio::{DispatchFailure, TapDispatcher};
use hq_types::hq::TapId;
use zakofish4_common::messages::ResponseVariant;
use zakofish4_common::state::PendingRequest;

use super::dispatch::DispatchError;
use super::Gateway;

pub struct GatewayDispatcher {
    gateway: Gateway,
}

impl GatewayDispatcher {
    pub fn new(gateway: Gateway) -> Self {
        Self { gateway }
    }
}

#[async_trait]
impl TapDispatcher for GatewayDispatcher {
    async fn dispatch(
        &self,
        tap_id: &TapId,
        pending: PendingRequest,
    ) -> Result<ResponseVariant, DispatchFailure> {
        // Cloned per attempt because the gateway retries on a stale presence
        // entry, and a retry has to carry the same request id — the sink is
        // already armed for exactly that one.
        self.gateway
            .request(tap_id, || pending.clone())
            .await
            .map_err(|e| match e {
                DispatchError::NoConnection(_) => DispatchFailure::NoConnection,
                DispatchError::Timeout => DispatchFailure::Timeout,
                DispatchError::Disconnected => DispatchFailure::Disconnected,
                DispatchError::Transport(m) => DispatchFailure::Transport(m),
            })
    }
}
