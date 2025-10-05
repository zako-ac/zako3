use async_trait::async_trait;
use mockall::automock;

use crate::{
    feature::settings::{Settings, SettingsObject, scope::SettingsScope},
    util::error::AppResult,
};

#[automock]
#[async_trait]
pub trait SettingsRepository {
    async fn get_settings(&self, scope: &SettingsScope) -> AppResult<Option<SettingsObject>>;

    /// Insert or update settings.
    async fn set_settings(&self, settings: &Settings) -> AppResult<()>;

    async fn delete_settings(&self, scope: &SettingsScope) -> AppResult<()>;
}
