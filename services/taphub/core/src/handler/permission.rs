use zako3_types::TapHubError;
use zako3_types::hq::{Tap, DiscordUserId};

use crate::hub::TapHub;

pub(crate) async fn verify_permission(
    tap_hub: &TapHub,
    tap: &Tap,
    discord_user_id: &DiscordUserId,
) -> Result<(), TapHubError> {
    if tap_hub.app.bypass_hq {
        return Ok(());
    }
    let allowed = tap_hub
        .app
        .hq_repository
        .verify_tap_permission(&tap.id.0, discord_user_id)
        .await;
    if allowed {
        Ok(())
    } else {
        Err(TapHubError::PermissionDenied(tap.id.0.clone()))
    }
}
