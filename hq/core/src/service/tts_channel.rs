use crate::{CoreResult, repo::TtsChannelRepo};
use hq_types::{ChannelId, GuildId};
use std::sync::Arc;

#[derive(Clone)]
pub struct TTSChannelService {
    repo: Arc<dyn TtsChannelRepo>,
}

impl TTSChannelService {
    pub fn new(repo: Arc<dyn TtsChannelRepo>) -> Self {
        Self { repo }
    }

    pub async fn set_enabled(
        &self,
        guild_id: &GuildId,
        channel_id: &ChannelId,
        enabled: bool,
    ) -> CoreResult<()> {
        self.repo.set_enabled(guild_id, channel_id, enabled).await
    }

    pub async fn is_enabled(&self, channel_id: &ChannelId) -> CoreResult<bool> {
        self.repo.is_enabled(channel_id).await
    }

    pub async fn get_enabled_channels(&self, guild_id: &GuildId) -> CoreResult<Vec<ChannelId>> {
        self.repo.get_enabled_channels(guild_id).await
    }
}
