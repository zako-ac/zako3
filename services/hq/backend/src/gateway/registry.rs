//! The connections this replica is holding, and who is waiting on them.
//!
//! Authoritative for *this process only*. Redis carries a TTL-leased
//! projection so other replicas can find these connections, but it is a
//! projection: if the two disagree, this is right and Redis is stale.

use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;
use tokio::sync::oneshot;
use hq_types::hq::TapId;
use hq_types::{OnlineTapState, OnlineTapStates};
use zakofish4_common::event::RequestOutcome;
use zakofish4_common::model::RequestId;
use zakofish4_hub::TapHandle;

/// One live tap connection.
#[derive(Clone)]
pub struct Connection {
    pub state: OnlineTapState,
    pub handle: TapHandle,
}

#[derive(Default)]
pub struct Registry {
    /// Keyed by connection id, which is unique within this process.
    connections: DashMap<u64, Connection>,
    /// Callers blocked on a dispatched request.
    waiters: DashMap<RequestId, oneshot::Sender<RequestOutcome>>,
    next_connection_id: AtomicU64,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            next_connection_id: AtomicU64::new(1),
            ..Default::default()
        }
    }

    pub fn next_connection_id(&self) -> u64 {
        self.next_connection_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn insert(&self, connection_id: u64, connection: Connection) {
        self.connections.insert(connection_id, connection);
    }

    pub fn remove(&self, connection_id: u64) -> Option<Connection> {
        self.connections.remove(&connection_id).map(|(_, c)| c)
    }

    pub fn get(&self, connection_id: u64) -> Option<Connection> {
        self.connections.get(&connection_id).map(|c| c.clone())
    }

    /// This replica's live connections for one tap, in the shape the presence
    /// projection publishes.
    pub fn states_for(&self, tap_id: &TapId) -> OnlineTapStates {
        self.connections
            .iter()
            .filter(|c| c.value().state.tap_id == *tap_id)
            .map(|c| c.value().state.clone())
            .collect()
    }

    /// Every tap this replica currently holds a connection for.
    pub fn tap_ids(&self) -> Vec<TapId> {
        let mut ids: Vec<TapId> = self
            .connections
            .iter()
            .map(|c| c.value().state.tap_id.clone())
            .collect();
        ids.sort_by(|a, b| a.0.cmp(&b.0));
        ids.dedup_by(|a, b| a.0 == b.0);
        ids
    }

    pub fn len(&self) -> usize {
        self.connections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    /// Register interest in a request's result before dispatching it.
    pub fn await_request(&self, request_id: RequestId) -> oneshot::Receiver<RequestOutcome> {
        let (tx, rx) = oneshot::channel();
        self.waiters.insert(request_id, tx);
        rx
    }

    /// Deliver a result to whoever is waiting.
    ///
    /// Returns `false` when nobody is — which is normal rather than an error:
    /// an audio request completes twice, once when the tap answers and again
    /// when the transfer ends, and only the first has a caller blocked on it.
    /// The second is for history and metrics.
    pub fn complete(&self, request_id: RequestId, outcome: RequestOutcome) -> bool {
        match self.waiters.remove(&request_id) {
            Some((_, tx)) => tx.send(outcome).is_ok(),
            None => false,
        }
    }

    /// Give up on a request, so a caller that timed out does not leak a slot.
    pub fn forget(&self, request_id: RequestId) {
        self.waiters.remove(&request_id);
    }

    pub fn pending_len(&self) -> usize {
        self.waiters.len()
    }
}
