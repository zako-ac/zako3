use std::sync::Arc;

use hq_types::hq::settings::{TextReadingRule, UserJoinLeaveAlert, UserSettings};
use hq_types::hq::UserId;
use zako3_states::UserSettingsStateService;

use crate::repo::UserRepository;
use crate::CoreResult;

fn default_settings() -> UserSettings {
    UserSettings {
        text_mappings: vec![],
        emoji_mappings: vec![],
        text_reading_rule: TextReadingRule::Always,
        user_join_leave_alert: UserJoinLeaveAlert::Auto,
        max_message_length: 100,
        enable_tts_queue: true,
        tts_voice: None,
    }
}

#[derive(Clone)]
pub struct UserSettingsService {
    user_repo: Arc<dyn UserRepository>,
    cache: UserSettingsStateService,
}

impl UserSettingsService {
    pub fn new(user_repo: Arc<dyn UserRepository>, cache: UserSettingsStateService) -> Self {
        Self { user_repo, cache }
    }

    pub async fn get_settings(&self, user_id: UserId) -> CoreResult<UserSettings> {
        if let Some(cached) = self.cache.get_cached(&user_id).await {
            return Ok(cached);
        }

        let settings = self
            .user_repo
            .get_settings(user_id.clone())
            .await?
            .unwrap_or_else(default_settings);

        self.cache.set_cached(&user_id, &settings).await;
        Ok(settings)
    }

    pub async fn save_settings(
        &self,
        user_id: UserId,
        settings: UserSettings,
    ) -> CoreResult<UserSettings> {
        let saved = self
            .user_repo
            .save_settings(user_id.clone(), &settings)
            .await?;
        self.cache.invalidate(&user_id).await;
        Ok(saved)
    }
}
