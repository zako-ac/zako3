use hq_core::CoreError;
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
    pub fn to_user_message(&self) -> &str {
        match self {
            BotError::NotInVoiceChannel => "이 명령어를 사용하려면 음성 채널에 있어야 해요.",
            BotError::BotNotInVoiceChannel => "저는 현재 음성 채널에 있지 않아요.",
            BotError::UserNotInSession => "제가 활동 중인 음성 채널에 있지 않으세요.",
            BotError::InvalidVolume => "볼륨은 0에서 150 사이여야 해요.",
            BotError::Forbidden => "이 작업을 수행할 권한이 없어요.",
            BotError::Unauthorized => {
                "먼저 로그인이 필요해요. `/settings`에서 로그인 링크를 확인하세요."
            }
            BotError::NothingPlaying => "현재 재생 중인 항목이 없어요.",
            BotError::QueueEmpty => "대기열이 비어 있어요.",
            BotError::ShouldRunInGuild => "이 명령어는 서버에서만 사용할 수 있어요.",
            BotError::Core(inner) => core_error_message(inner),
            BotError::Serenity(_) => "Discord와 통신하는 중 문제가 발생했어요.",
            BotError::State(_) => "서버에서 문제가 발생했어요. 다시 시도해 주세요.",
            BotError::Other(msg) => msg.as_str(),
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
            ) | BotError::Serenity(_)
                | BotError::State(_)
        )
    }
}

fn core_error_message(err: &CoreError) -> &str {
    match err {
        CoreError::NotFound(_) => "해당 항목을 찾을 수 없어요.",
        CoreError::Unauthorized(_) => {
            "먼저 로그인이 필요해요. `/settings`에서 로그인 링크를 확인하세요."
        }
        CoreError::Forbidden(_) => "이 작업을 수행할 권한이 없어요.",
        CoreError::Conflict(msg) => msg.as_str(),
        CoreError::InvalidInput(msg) => msg.as_str(),
        _ => "서버에서 문제가 발생했어요. 다시 시도해 주세요.",
    }
}

impl From<String> for BotError {
    fn from(s: String) -> Self {
        BotError::Other(s)
    }
}
