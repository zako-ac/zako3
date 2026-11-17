use zako3_types::hq::{DiscordUserId, Tap};
use zako3_types::{TapHubError, ZakoError};

use crate::hub::TapHub;

pub(crate) async fn verify_permission(
    tap_hub: &TapHub,
    tap: &Tap,
    discord_user_id: &DiscordUserId,
) -> Result<(), TapHubError> {
    if tap_hub.app.bypass_hq {
        return Ok(());
    }
    let lookup = tap_hub
        .app
        .hq_repository
        .verify_tap_permission(&tap.id.0, discord_user_id)
        .await;
    classify_lookup(&tap.id.0, lookup)
}

/// Turn HQ's answer (or its absence) into a tap-hub error.
///
/// `Ok(true)`/`Ok(false)` is HQ saying yes or no. An `Err` is HQ not answering
/// at all, which says nothing about the user's rights: reporting it as
/// `PermissionDenied` used to delete the track and lose the message on a
/// 5-second RPC blip. It becomes `Internal`, the only shape a caller can
/// retry -- on the play path that caller is HQ's own play retry in `hq-core`'s
/// `PlaybackService::play_with_retry`. The taphub transport never retries an
/// application error, and an inline retry here could not fit the engine's
/// per-attempt budget anyway (a 5 s RPC against a 6 s engine attempt).
fn classify_lookup(tap_id: &str, lookup: Result<bool, ZakoError>) -> Result<(), TapHubError> {
    match lookup {
        Ok(true) => Ok(()),
        Ok(false) => Err(TapHubError::PermissionDenied(tap_id.to_string())),
        Err(err) => {
            tracing::warn!(
                tap_id,
                error = %err,
                "tap permission lookup failed; treating as a transient error, not a denial"
            );
            Err(TapHubError::Internal(format!(
                "permission lookup failed for tap {tap_id}: {err}"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_when_hq_says_yes() {
        assert!(classify_lookup("tap-1", Ok(true)).is_ok());
    }

    #[test]
    fn denied_only_when_hq_says_no() {
        let err = classify_lookup("tap-1", Ok(false)).unwrap_err();
        assert!(matches!(err, TapHubError::PermissionDenied(id) if id == "tap-1"));
    }

    #[test]
    fn a_failed_lookup_is_not_a_permission_denial() {
        let err =
            classify_lookup("tap-1", Err(ZakoError::Rpc("request timeout".into()))).unwrap_err();
        match err {
            TapHubError::PermissionDenied(_) => {
                panic!("a lookup failure must not read as a denial")
            }
            TapHubError::Internal(msg) => {
                assert!(msg.contains("permission lookup failed"), "{msg}");
                assert!(msg.contains("tap-1"), "{msg}");
                assert!(msg.contains("request timeout"), "{msg}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
