use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{TapName, hq::TapId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnlineTapState {
    pub tap_id: TapId,
    pub tap_name: TapName,
    pub connection_id: u64,
    pub friendly_name: String,
    pub selection_weight: f32,
    pub connected_at: DateTime<Utc>,
    /// Which process holds this connection.
    ///
    /// Empty for the single-writer taphub, which is why it defaults rather than
    /// being required. The HQ gateway runs multi-replica, so a reader has to
    /// know which replica to route a dispatch to — an id alone is not enough.
    #[serde(default)]
    pub replica_id: String,
}

pub type OnlineTapStates = Vec<OnlineTapState>;
