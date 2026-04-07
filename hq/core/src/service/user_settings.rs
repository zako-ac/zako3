use std::sync::Arc;

use hq_types::hq::settings::{PartialUserSettings, UserSettings};
use hq_types::hq::UserId;
use zako3_states::UserSettingsStateService;

use crate::repo::{GlobalSettingsRepository, GuildSettingsRepository, UserGuildSettingsRepository, UserRepository};
use crate::CoreResult;

#[derive(Clone)]
pub struct UserSettingsService {
    user_repo: Arc<dyn UserRepository>,
    guild_settings_repo: Arc<dyn GuildSettingsRepository>,
    user_guild_settings_repo: Arc<dyn UserGuildSettingsRepository>,
    global_settings_repo: Arc<dyn GlobalSettingsRepository>,
    cache: UserSettingsStateService,
}

impl UserSettingsService {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        guild_settings_repo: Arc<dyn GuildSettingsRepository>,
        user_guild_settings_repo: Arc<dyn UserGuildSettingsRepository>,
        global_settings_repo: Arc<dyn GlobalSettingsRepository>,
        cache: UserSettingsStateService,
    ) -> Self {
        Self {
            user_repo,
            guild_settings_repo,
            user_guild_settings_repo,
            global_settings_repo,
            cache,
        }
    }

    // --- User scope ---

    pub async fn get_settings(&self, user_id: UserId) -> CoreResult<PartialUserSettings> {
        if let Some(cached) = self.cache.get_user_cached(&user_id).await {
            return Ok(cached);
        }

        let settings = self
            .user_repo
            .get_settings(user_id.clone())
            .await?
            .unwrap_or_else(PartialUserSettings::empty);

        self.cache.set_user_cached(&user_id, &settings).await;
        Ok(settings)
    }

    pub async fn save_settings(
        &self,
        user_id: UserId,
        settings: PartialUserSettings,
    ) -> CoreResult<PartialUserSettings> {
        let saved = self
            .user_repo
            .save_settings(user_id.clone(), &settings)
            .await?;
        self.cache.invalidate_user(&user_id).await;
        Ok(saved)
    }

    // --- GuildUser scope ---

    pub async fn get_guild_user_settings(
        &self,
        user_id: &UserId,
        guild_id: &str,
    ) -> CoreResult<Option<PartialUserSettings>> {
        if let Some(cached) = self.cache.get_guild_user_cached(user_id, guild_id).await {
            return Ok(Some(cached));
        }

        let settings = self
            .user_guild_settings_repo
            .get(user_id, guild_id)
            .await?;

        if let Some(ref s) = settings {
            self.cache.set_guild_user_cached(user_id, guild_id, s).await;
        }

        Ok(settings)
    }

    pub async fn save_guild_user_settings(
        &self,
        user_id: &UserId,
        guild_id: &str,
        settings: PartialUserSettings,
    ) -> CoreResult<PartialUserSettings> {
        let saved = self
            .user_guild_settings_repo
            .upsert(user_id, guild_id, &settings)
            .await?;
        self.cache.invalidate_guild_user(user_id, guild_id).await;
        Ok(saved)
    }

    pub async fn delete_guild_user_settings(
        &self,
        user_id: &UserId,
        guild_id: &str,
    ) -> CoreResult<()> {
        self.user_guild_settings_repo.delete(user_id, guild_id).await?;
        self.cache.invalidate_guild_user(user_id, guild_id).await;
        Ok(())
    }

    // --- Guild scope ---

    pub async fn get_guild_settings(
        &self,
        guild_id: &str,
    ) -> CoreResult<Option<PartialUserSettings>> {
        if let Some(cached) = self.cache.get_guild_cached(guild_id).await {
            return Ok(Some(cached));
        }

        let settings = self.guild_settings_repo.get(guild_id).await?;

        if let Some(ref s) = settings {
            self.cache.set_guild_cached(guild_id, s).await;
        }

        Ok(settings)
    }

    pub async fn save_guild_settings(
        &self,
        guild_id: &str,
        settings: PartialUserSettings,
    ) -> CoreResult<PartialUserSettings> {
        let saved = self.guild_settings_repo.upsert(guild_id, &settings).await?;
        self.cache.invalidate_guild(guild_id).await;
        Ok(saved)
    }

    // --- Global scope ---

    pub async fn get_global_settings(&self) -> CoreResult<Option<PartialUserSettings>> {
        if let Some(cached) = self.cache.get_global_cached().await {
            return Ok(Some(cached));
        }

        let settings = self.global_settings_repo.get().await?;

        if let Some(ref s) = settings {
            self.cache.set_global_cached(s).await;
        }

        Ok(settings)
    }

    pub async fn save_global_settings(
        &self,
        settings: PartialUserSettings,
    ) -> CoreResult<PartialUserSettings> {
        let saved = self.global_settings_repo.upsert(&settings).await?;
        self.cache.invalidate_global().await;
        Ok(saved)
    }

    // --- Effective (resolved) settings ---

    /// Fetch all four scopes concurrently and fold them into a concrete `UserSettings`.
    /// Cascade: GuildUser > User > Guild > Global > hardcoded defaults.
    /// If `guild_id` is `None`, only `User`, `Global`, and hardcoded defaults are used.
    pub async fn get_effective_settings(
        &self,
        user_id: &UserId,
        guild_id: Option<&str>,
    ) -> CoreResult<UserSettings> {
        let (guild_user, user, guild, global) = tokio::try_join!(
            async {
                if let Some(gid) = guild_id {
                    self.user_guild_settings_repo.get(user_id, gid).await
                } else {
                    Ok(None)
                }
            },
            self.user_repo.get_settings(user_id.clone()),
            async {
                if let Some(gid) = guild_id {
                    self.guild_settings_repo.get(gid).await
                } else {
                    Ok(None)
                }
            },
            self.global_settings_repo.get(),
        )?;

        let guild_user = guild_user.unwrap_or_else(PartialUserSettings::empty);
        let user = user.unwrap_or_else(PartialUserSettings::empty);
        let guild = guild.unwrap_or_else(PartialUserSettings::empty);
        let global = global.unwrap_or_else(PartialUserSettings::empty);

        let merged = PartialUserSettings::fold(
            &guild_user,
            &PartialUserSettings::fold(
                &user,
                &PartialUserSettings::fold(&guild, &global),
            ),
        );

        Ok(merged.resolve())
    }
}
