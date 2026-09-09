//! Getting a request to whichever replica holds the tap's connection.
//!
//! A tap's WebSocket lands on one replica, but the audio engine's RPC may hit
//! any of them. Presence in Redis says which replica owns which connection;
//! NATS carries the request there and the answer back.
//!
//! NATS rather than a hand-rolled Redis pub/sub inbox because request/reply,
//! correlation and timeouts come with it — and because "no responders" fires in
//! microseconds, so a dead replica is discovered immediately instead of after
//! the full timeout.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use hq_types::hq::TapId as HqTapId;
use zakofish4_common::event::RequestOutcome;
use zakofish4_common::messages::{RequestVariant, ResponseVariant};
use zakofish4_common::model::RequestId;
use zakofish4_common::state::PendingRequest;

use super::GatewayShared;

/// Subject a replica listens on for one of its connections.
///
/// Per connection rather than per tap, because selection is weighted: a queue
/// group on the tap would load-balance and throw the weights away.
pub fn subject(replica_id: &str, connection_id: u64) -> String {
    format!("hq.gw.{replica_id}.{connection_id}")
}

/// One replica asking another to serve a request.
#[derive(Debug, Serialize, Deserialize)]
pub struct DispatchRequest {
    pub connection_id: u64,
    pub request_id: RequestId,
    pub variant: RequestVariant,
    pub timeout_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DispatchReply {
    Answered(ResponseVariant),
    /// The tap never answered in time.
    TimedOut,
    /// The connection went away before the tap answered.
    Disconnected,
    /// That replica no longer holds this connection — its presence entry is
    /// stale and the caller should try another.
    UnknownConnection,
}

#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error("tap {0} has no connected instance")]
    NoConnection(String),

    #[error("the tap did not answer in time")]
    Timeout,

    #[error("the tap's connection closed before it answered")]
    Disconnected,

    #[error("cross-replica dispatch failed: {0}")]
    Transport(String),
}

/// Serve a request on a connection this replica holds.
///
/// The waiter is registered *before* the request goes out, so an answer that
/// arrives immediately cannot race past it.
pub async fn dispatch_local(
    shared: &Arc<GatewayShared>,
    connection_id: u64,
    pending: PendingRequest,
) -> Result<ResponseVariant, DispatchError> {
    let Some(conn) = shared.registry.get(connection_id) else {
        return Err(DispatchError::NoConnection(format!("conn {connection_id}")));
    };

    let request_id = pending.request_id;
    let timeout = pending.timeout;
    let rx = shared.registry.await_request(request_id);

    if !conn.handle.dispatch(pending) {
        shared.registry.forget(request_id);
        return Err(DispatchError::Disconnected);
    }

    // The state machine already times the request out and reports it, so this
    // is only a backstop against the connection task dying outright. Give it
    // headroom so the specific error wins over the generic one.
    match tokio::time::timeout(timeout + Duration::from_secs(2), rx).await {
        Ok(Ok(RequestOutcome::Answered(v))) => Ok(v),
        Ok(Ok(RequestOutcome::TimedOut)) => Err(DispatchError::Timeout),
        Ok(Ok(RequestOutcome::Disconnected)) => Err(DispatchError::Disconnected),
        Ok(Ok(RequestOutcome::Streamed(_))) => {
            // A stream report cannot be the first completion of a request.
            Err(DispatchError::Transport("unexpected stream outcome".into()))
        }
        Ok(Err(_)) => {
            shared.registry.forget(request_id);
            Err(DispatchError::Disconnected)
        }
        Err(_) => {
            shared.registry.forget(request_id);
            Err(DispatchError::Timeout)
        }
    }
}

/// Ask the replica that owns `connection_id` to serve this request.
pub async fn dispatch_remote(
    shared: &Arc<GatewayShared>,
    replica_id: &str,
    connection_id: u64,
    pending: PendingRequest,
) -> Result<ResponseVariant, DispatchError> {
    let Some(nats) = shared.nats.as_ref() else {
        // Single-replica deployments have no NATS, and a presence entry naming
        // another replica then means stale state rather than a live peer.
        return Err(DispatchError::Transport(
            "no NATS client; cannot reach another replica".into(),
        ));
    };

    let payload = serde_json::to_vec(&DispatchRequest {
        connection_id,
        request_id: pending.request_id,
        variant: pending.variant,
        timeout_ms: pending.timeout.as_millis() as u64,
    })
    .map_err(|e| DispatchError::Transport(e.to_string()))?;

    let msg = tokio::time::timeout(
        pending.timeout + Duration::from_secs(3),
        nats.request(subject(replica_id, connection_id), payload.into()),
    )
    .await
    .map_err(|_| DispatchError::Timeout)?
    .map_err(|e| DispatchError::Transport(e.to_string()))?;

    match serde_json::from_slice::<DispatchReply>(&msg.payload)
        .map_err(|e| DispatchError::Transport(e.to_string()))?
    {
        DispatchReply::Answered(v) => Ok(v),
        DispatchReply::TimedOut => Err(DispatchError::Timeout),
        DispatchReply::Disconnected => Err(DispatchError::Disconnected),
        DispatchReply::UnknownConnection => {
            Err(DispatchError::NoConnection(format!("conn {connection_id}")))
        }
    }
}

/// Answer dispatch requests aimed at this replica's connections.
pub fn serve_subject(shared: Arc<GatewayShared>, connection_id: u64) {
    let Some(nats) = shared.nats.clone() else {
        return;
    };
    let subject = subject(&shared.replica_id, connection_id);

    tokio::spawn(async move {
        let mut sub = match nats.subscribe(subject.clone()).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(%e, %subject, "failed to subscribe for cross-replica dispatch");
                return;
            }
        };

        use futures_util::StreamExt;
        while let Some(msg) = sub.next().await {
            let Some(reply_to) = msg.reply.clone() else {
                continue;
            };
            let shared = Arc::clone(&shared);
            let nats = nats.clone();
            // Per message, so one slow tap does not block this connection's
            // other requests behind it.
            tokio::spawn(async move {
                let reply = handle_dispatch(&shared, &msg.payload).await;
                let payload = serde_json::to_vec(&reply).unwrap_or_default();
                let _ = nats.publish(reply_to, payload.into()).await;
            });
        }
    });
}

async fn handle_dispatch(shared: &Arc<GatewayShared>, payload: &[u8]) -> DispatchReply {
    let Ok(req) = serde_json::from_slice::<DispatchRequest>(payload) else {
        return DispatchReply::UnknownConnection;
    };

    let pending = PendingRequest {
        request_id: req.request_id,
        variant: req.variant,
        timeout: Duration::from_millis(req.timeout_ms),
    };

    match dispatch_local(shared, req.connection_id, pending).await {
        Ok(v) => DispatchReply::Answered(v),
        Err(DispatchError::Timeout) => DispatchReply::TimedOut,
        Err(DispatchError::Disconnected) => DispatchReply::Disconnected,
        Err(DispatchError::NoConnection(_)) => DispatchReply::UnknownConnection,
        Err(DispatchError::Transport(e)) => {
            tracing::warn!(%e, "cross-replica dispatch failed locally");
            DispatchReply::Disconnected
        }
    }
}

/// Cancel a request. Best effort in both senses: the tap may not hear it, and
/// the owning replica may be gone.
pub async fn cancel(shared: &Arc<GatewayShared>, tap_id: &HqTapId, request_id: RequestId) {
    let states = shared.presence.get(tap_id).await.unwrap_or_default();
    for state in states {
        if state.replica_id == shared.replica_id {
            if let Some(conn) = shared.registry.get(state.connection_id) {
                conn.handle.cancel(request_id);
            }
        } else if let Some(nats) = shared.nats.as_ref() {
            let subject = format!("hq.gw.cancel.{}", state.replica_id);
            let payload = serde_json::to_vec(&(state.connection_id, request_id))
                .unwrap_or_default();
            let _ = nats.publish(subject, payload.into()).await;
        }
    }
}
