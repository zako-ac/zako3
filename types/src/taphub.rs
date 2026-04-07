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
}

pub type OnlineTapStates = Vec<OnlineTapState>;
