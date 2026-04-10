use zako3_types::hq::{Tap, TapPermission, DiscordUserId};

use crate::hub::TapHub;

pub(crate) async fn verify_permission(
    tap_hub: &TapHub,
    tap: &Tap,
    discord_user_id: &DiscordUserId,
) -> Result<(), String> {
    match &tap.permission {
        TapPermission::Public => Ok(()),
        TapPermission::OwnerOnly => {
            let user = tap_hub
                .app
                .hq_repository
                .get_user_by_discord_id(discord_user_id)
                .await
                .ok_or_else(|| "User not found in HQ".to_string())?;

            if user.id == tap.owner_id {
                Ok(())
            } else {
                Err(format!("User is not the owner of tap {}", tap.id.0))
            }
        }
        TapPermission::Whitelisted { user_ids } => {
            if user_ids.contains(&discord_user_id.0) {
                Ok(())
            } else {
                Err(format!("User is not whitelisted for tap {}", tap.id.0))
            }
        }
        TapPermission::Blacklisted { user_ids } => {
            if user_ids.contains(&discord_user_id.0) {
                Err(format!("User is blacklisted for tap {}", tap.id.0))
            } else {
                Ok(())
            }
        }
    }
}
