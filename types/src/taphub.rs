use serde::{Deserialize, Serialize};

use crate::hq::TapId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnlineTapState {
    pub tap_id: TapId,
    pub friendly_name: String,
    pub selection_weight: f32,
}

pub type OnlineTapStates = Vec<OnlineTapState>;
