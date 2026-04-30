use zako3_types::hq::{Tap, DiscordUserId};

use crate::hub::TapHub;

pub(crate) async fn verify_permission(
    tap_hub: &TapHub,
    tap: &Tap,
    discord_user_id: &DiscordUserId,
) -> Result<(), String> {
    let allowed = tap_hub
        .app
        .hq_repository
        .verify_tap_permission(&tap.id.0, discord_user_id)
        .await;
    if allowed {
        Ok(())
    } else {
        Err(format!("Access denied for tap {}", tap.id.0))
    }
}
