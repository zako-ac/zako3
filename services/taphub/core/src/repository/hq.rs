use std::sync::Arc;

use async_trait::async_trait;
use zako3_types::hq::{DiscordUserId, Tap, User};

#[async_trait]
pub trait HqRepository: Send + Sync {
    async fn authenticate_tap(&self, token: &str) -> Option<Tap>;
    async fn get_tap_by_id(&self, tap_id: &str) -> Option<Tap>;
    async fn get_user_by_discord_id(&self, discord_id: &DiscordUserId) -> Option<User>;
}

pub type HqRepositoryRef = Arc<dyn HqRepository>;
