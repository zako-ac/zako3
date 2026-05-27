use zako3_types::TapHubError;
use zako3_types::hq::{Tap, TapId};

use crate::hub::TapHub;

pub(crate) async fn resolve_tap(tap_hub: &TapHub, tap_id: &TapId) -> Result<Tap, TapHubError> {
    if tap_hub.app.bypass_hq {
        return Ok(synthetic_tap(tap_id));
    }
    tap_hub
        .app
        .hq_repository
        .get_tap_by_id(&tap_id.0)
        .await
        .ok_or_else(|| TapHubError::TapNotFound(tap_id.0.clone()))
}

pub(crate) fn synthetic_tap(tap_id: &TapId) -> Tap {
    Tap::new(tap_id.0.clone(), "bypass", "bypass".to_string())
}
