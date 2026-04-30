use zako3_types::hq::{UserId, settings::PartialUserSettings};

use crate::cache_repo::CacheRepositoryRef;

#[derive(Clone)]
pub struct UserSettingsStateService {
    cache_repository: CacheRepositoryRef,
}

impl UserSettingsStateService {
    pub fn new(cache_repository: CacheRepositoryRef) -> Self {
        Self { cache_repository }
    }

    fn user_key(user_id: &UserId) -> String {
        format!("user_settings:{}", user_id.0)
    }

    fn guild_user_key(user_id: &UserId, guild_id: &str) -> String {
        format!("user_guild_settings:{}:{}", user_id.0, guild_id)
    }

    fn guild_key(guild_id: &str) -> String {
        format!("guild_settings:{}", guild_id)
    }

    fn global_key() -> &'static str {
        "global_settings"
    }

    // --- User scope ---

    pub async fn get_user_cached(&self, user_id: &UserId) -> Option<PartialUserSettings> {
        let raw = self.cache_repository.get(&Self::user_key(user_id)).await?;
        serde_json::from_str(&raw).ok()
    }

    pub async fn set_user_cached(&self, user_id: &UserId, settings: &PartialUserSettings) {
        if let Ok(raw) = serde_json::to_string(settings) {
            self.cache_repository.set(&Self::user_key(user_id), &raw).await;
        }
    }

    pub async fn invalidate_user(&self, user_id: &UserId) {
        self.cache_repository.del(&Self::user_key(user_id)).await;
    }

    // --- GuildUser scope ---

    pub async fn get_guild_user_cached(&self, user_id: &UserId, guild_id: &str) -> Option<PartialUserSettings> {
        let raw = self.cache_repository.get(&Self::guild_user_key(user_id, guild_id)).await?;
        serde_json::from_str(&raw).ok()
    }

    pub async fn set_guild_user_cached(&self, user_id: &UserId, guild_id: &str, settings: &PartialUserSettings) {
        if let Ok(raw) = serde_json::to_string(settings) {
            self.cache_repository.set(&Self::guild_user_key(user_id, guild_id), &raw).await;
        }
    }

    pub async fn invalidate_guild_user(&self, user_id: &UserId, guild_id: &str) {
        self.cache_repository.del(&Self::guild_user_key(user_id, guild_id)).await;
    }

    // --- Guild scope ---

    pub async fn get_guild_cached(&self, guild_id: &str) -> Option<PartialUserSettings> {
        let raw = self.cache_repository.get(&Self::guild_key(guild_id)).await?;
        serde_json::from_str(&raw).ok()
    }

    pub async fn set_guild_cached(&self, guild_id: &str, settings: &PartialUserSettings) {
        if let Ok(raw) = serde_json::to_string(settings) {
            self.cache_repository.set(&Self::guild_key(guild_id), &raw).await;
        }
    }

    pub async fn invalidate_guild(&self, guild_id: &str) {
        self.cache_repository.del(&Self::guild_key(guild_id)).await;
    }

    // --- Global scope ---

    pub async fn get_global_cached(&self) -> Option<PartialUserSettings> {
        let raw = self.cache_repository.get(Self::global_key()).await?;
        serde_json::from_str(&raw).ok()
    }

    pub async fn set_global_cached(&self, settings: &PartialUserSettings) {
        if let Ok(raw) = serde_json::to_string(settings) {
            self.cache_repository.set(Self::global_key(), &raw).await;
        }
    }

    pub async fn invalidate_global(&self) {
        self.cache_repository.del(Self::global_key()).await;
    }
}
