use hq_core::CoreError;
use poise::serenity_prelude as serenity;
use thiserror::Error;
use zako3_states::StateServiceError;

#[derive(Debug, Error)]
pub enum BotError {
    // ── User-facing domain errors ───────────────────────────────────────────
    #[error("You need to be in a voice channel to use this command.")]
    NotInVoiceChannel,
    #[error("I'm not currently in a voice channel.")]
    BotNotInVoiceChannel,
    #[error("Volume must be between 0 and 150.")]
    InvalidVolume,
    #[error("You don't have permission to do that.")]
    Forbidden,
    #[error("You need to log in first. Use `/settings` to get a login link.")]
    Unauthorized,
    #[error("Nothing is currently playing.")]
    NothingPlaying,
    #[error("The queue is empty.")]
    QueueEmpty,
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
            BotError::NotInVoiceChannel => "You need to be in a voice channel to use this command.",
            BotError::BotNotInVoiceChannel => "I'm not currently in a voice channel.",
            BotError::InvalidVolume => "Volume must be between 0 and 150.",
            BotError::Forbidden => "You don't have permission to do that.",
            BotError::Unauthorized => {
                "You need to log in first. Use `/settings` to get a login link."
            }
            BotError::NothingPlaying => "Nothing is currently playing.",
            BotError::QueueEmpty => "The queue is empty.",
            BotError::Core(inner) => core_error_message(inner),
            BotError::Serenity(_) => "Something went wrong communicating with Discord.",
            BotError::State(_) => "Something went wrong on our end. Please try again.",
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
        CoreError::NotFound(_) => "That item couldn't be found.",
        CoreError::Unauthorized(_) => {
            "You need to log in first. Use `/settings` to get a login link."
        }
        CoreError::Forbidden(_) => "You don't have permission to do that.",
        CoreError::Conflict(_) => "That's already been set up.",
        CoreError::InvalidInput(msg) => msg.as_str(),
        _ => "Something went wrong on our end. Please try again.",
    }
}

impl From<String> for BotError {
    fn from(s: String) -> Self {
        BotError::Other(s)
    }
}
