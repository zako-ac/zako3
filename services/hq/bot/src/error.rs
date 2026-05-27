use std::borrow::Cow;

use hq_core::CoreError;
use hq_types::TapHubError;
use poise::serenity_prelude as serenity;
use thiserror::Error;
use zako3_states::StateServiceError;

#[derive(Debug, Error)]
pub enum BotError {
    // ── User-facing domain errors ───────────────────────────────────────────
    #[error("이 명령어를 사용하려면 음성 채널에 있어야 해요.")]
    NotInVoiceChannel,
    #[error("저는 현재 음성 채널에 있지 않아요.")]
    BotNotInVoiceChannel,
    #[error("제가 활동 중인 음성 채널에 있지 않으세요.")]
    UserNotInSession,
    #[error("볼륨은 0에서 150 사이여야 해요.")]
    InvalidVolume,
    #[error("이 작업을 수행할 권한이 없어요.")]
    Forbidden,
    #[error("먼저 로그인이 필요해요. `/settings`에서 로그인 링크를 확인하세요.")]
    Unauthorized,
    #[error("현재 재생 중인 항목이 없어요.")]
    NothingPlaying,
    #[error("대기열이 비어 있어요.")]
    QueueEmpty,
    #[error("이 명령어는 서버에서만 사용할 수 있어요.")]
    ShouldRunInGuild,
    // ── Transparent infrastructure errors ──────────────────────────────────
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error(transparent)]
    Serenity(#[from] serenity::Error),
    #[error(transparent)]
    State(#[from] StateServiceError),
    #[error("{0}")]
    Other(String),
}

impl BotError {
    /// Returns a short, user-friendly message suitable for a Discord reply.
    /// Uses `Cow` because some variants (e.g. `TapHubError::TapScript`)
    /// produce an owned string rather than borrowing from `self`.
    pub fn to_user_message(&self) -> Cow<'_, str> {
        match self {
            BotError::NotInVoiceChannel => "이 명령어를 사용하려면 음성 채널에 있어야 해요.".into(),
            BotError::BotNotInVoiceChannel => "저는 현재 음성 채널에 있지 않아요.".into(),
            BotError::UserNotInSession => "제가 활동 중인 음성 채널에 있지 않으세요.".into(),
            BotError::InvalidVolume => "볼륨은 0에서 150 사이여야 해요.".into(),
            BotError::Forbidden => "이 작업을 수행할 권한이 없어요.".into(),
            BotError::Unauthorized => {
                "먼저 로그인이 필요해요. `/settings`에서 로그인 링크를 확인하세요.".into()
            }
            BotError::NothingPlaying => "현재 재생 중인 항목이 없어요.".into(),
            BotError::QueueEmpty => "대기열이 비어 있어요.".into(),
            BotError::ShouldRunInGuild => "이 명령어는 서버에서만 사용할 수 있어요.".into(),
            BotError::Core(inner) => core_error_message(inner),
            BotError::Serenity(_) => "Discord와 통신하는 중 문제가 발생했어요.".into(),
            BotError::State(_) => "서버에서 문제가 발생했어요. 다시 시도해 주세요.".into(),
            BotError::Other(msg) => msg.as_str().into(),
        }
    }

    /// Returns true for errors that indicate a programming/infrastructure fault
    /// (as opposed to expected user-facing errors).
    pub fn is_internal(&self) -> bool {
        matches!(
            self,
            BotError::Core(
                CoreError::DbError(_)
                    | CoreError::DbMigrationError(_)
                    | CoreError::ConfigError(_)
                    | CoreError::EnvError(_)
                    | CoreError::ReqwestError(_)
                    | CoreError::JsonError(_)
                    | CoreError::JwtError(_)
                    | CoreError::Internal(_)
                    | CoreError::StateError(_)
                    | CoreError::TapHub(TapHubError::Internal(_)),
            ) | BotError::Serenity(_)
                | BotError::State(_)
        )
    }
}

fn core_error_message(err: &CoreError) -> Cow<'_, str> {
    match err {
        CoreError::NotFound(_) => "해당 항목을 찾을 수 없어요.".into(),
        CoreError::Unauthorized(_) => {
            "먼저 로그인이 필요해요. `/settings`에서 로그인 링크를 확인하세요.".into()
        }
        CoreError::Forbidden(_) => "이 작업을 수행할 권한이 없어요.".into(),
        CoreError::Conflict(msg) => msg.as_str().into(),
        CoreError::InvalidInput(msg) => msg.as_str().into(),
        CoreError::TapHub(t) => tap_hub_error_message(t),
        _ => "서버에서 문제가 발생했어요. 다시 시도해 주세요.".into(),
    }
}

fn tap_hub_error_message(err: &TapHubError) -> Cow<'_, str> {
    match err {
        TapHubError::TapUnavailable => "Tap에 연결할 수 없어요.".into(),
        TapHubError::TapNotFound(_) => "해당 Tap을 찾을 수 없어요.".into(),
        TapHubError::PermissionDenied(_) => "이 Tap을 사용할 권한이 없어요.".into(),
        TapHubError::TapScript { reason, .. } => format!("Tap에서 오류: {reason}").into(),
        TapHubError::Internal(_) => "서버에서 문제가 발생했어요. 다시 시도해 주세요.".into(),
    }
}

impl From<String> for BotError {
    fn from(s: String) -> Self {
        BotError::Other(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(err: TapHubError) -> String {
        BotError::Core(CoreError::TapHub(err))
            .to_user_message()
            .into_owned()
    }

    #[test]
    fn tap_unavailable_maps_to_korean() {
        assert_eq!(msg(TapHubError::TapUnavailable), "Tap에 연결할 수 없어요.");
    }

    #[test]
    fn tap_not_found_maps_to_korean() {
        assert_eq!(
            msg(TapHubError::TapNotFound("youtube".into())),
            "해당 Tap을 찾을 수 없어요."
        );
    }

    #[test]
    fn permission_denied_maps_to_korean() {
        assert_eq!(
            msg(TapHubError::PermissionDenied("youtube".into())),
            "이 Tap을 사용할 권한이 없어요."
        );
    }

    #[test]
    fn tap_script_includes_reason() {
        assert_eq!(
            msg(TapHubError::TapScript {
                reason: "video unavailable".into(),
                try_others: false,
            }),
            "Tap에서 오류: video unavailable"
        );
    }

    #[test]
    fn internal_falls_back_to_generic_message() {
        assert_eq!(
            msg(TapHubError::Internal("rmp decode failed".into())),
            "서버에서 문제가 발생했어요. 다시 시도해 주세요."
        );
    }

    #[test]
    fn structured_taphub_errors_are_not_internal() {
        let e = BotError::Core(CoreError::TapHub(TapHubError::TapUnavailable));
        assert!(!e.is_internal());

        let e = BotError::Core(CoreError::TapHub(TapHubError::TapScript {
            reason: "x".into(),
            try_others: false,
        }));
        assert!(!e.is_internal());
    }

    #[test]
    fn taphub_internal_is_internal() {
        let e = BotError::Core(CoreError::TapHub(TapHubError::Internal("x".into())));
        assert!(e.is_internal());
    }
}
