use std::sync::Arc;

use async_trait::async_trait;
use zako3_types::ZakoError;
use zako3_types::hq::{DiscordUserId, Tap, User};

#[async_trait]
pub trait HqRepository: Send + Sync {
    async fn authenticate_tap(&self, token: &str) -> Option<Tap>;
    async fn get_tap_by_id(&self, tap_id: &str) -> Option<Tap>;
    async fn get_user_by_discord_id(&self, discord_id: &DiscordUserId) -> Option<User>;
    /// Whether `discord_user_id` may use `tap_id`.
    ///
    /// `Ok(true)`/`Ok(false)` is HQ's answer. `Err` means the lookup itself
    /// failed (RPC down, timeout) and is *not* an answer about the user's
    /// rights, so callers must not turn it into a denial.
    async fn verify_tap_permission(
        &self,
        tap_id: &str,
        discord_user_id: &DiscordUserId,
    ) -> Result<bool, ZakoError>;
}

pub type HqRepositoryRef = Arc<dyn HqRepository>;
