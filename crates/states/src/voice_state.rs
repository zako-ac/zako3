use serde::{Deserialize, Serialize};

use crate::cache_repo::CacheRepositoryRef;
use crate::error::{Result, StateServiceError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceChannelLocation {
    pub guild_id: u64,
    pub channel_id: u64,
    #[serde(default)]
    pub guild_name: String,
    #[serde(default)]
    pub channel_name: String,
}

#[derive(Clone)]
pub struct VoiceStateService {
    cache_repository: CacheRepositoryRef,
}

impl VoiceStateService {
    pub fn new(cache_repository: CacheRepositoryRef) -> Self {
        Self { cache_repository }
    }

    fn key(discord_user_id: &str) -> String {
        format!("voice_state:{}", discord_user_id)
    }

    pub async fn set_user_channel(
        &self,
        discord_user_id: &str,
        guild_id: u64,
        channel_id: u64,
        guild_name: String,
        channel_name: String,
    ) -> Result<()> {
        let mut locations = self.get_user_channels(discord_user_id).await?;
        locations.retain(|loc| loc.guild_id != guild_id);
        locations.push(VoiceChannelLocation { guild_id, channel_id, guild_name, channel_name });
        let serialized =
            serde_json::to_string(&locations).map_err(|_| StateServiceError::CacheError)?;
        self.cache_repository.set(&Self::key(discord_user_id), &serialized).await;
        Ok(())
    }

    pub async fn remove_user_from_guild(
        &self,
        discord_user_id: &str,
        guild_id: u64,
    ) -> Result<()> {
        let mut locations = self.get_user_channels(discord_user_id).await?;
        locations.retain(|loc| loc.guild_id != guild_id);
        if locations.is_empty() {
            self.cache_repository.del(&Self::key(discord_user_id)).await;
        } else {
            let serialized =
                serde_json::to_string(&locations).map_err(|_| StateServiceError::CacheError)?;
            self.cache_repository.set(&Self::key(discord_user_id), &serialized).await;
        }
        Ok(())
    }

    pub async fn get_user_channels(
        &self,
        discord_user_id: &str,
    ) -> Result<Vec<VoiceChannelLocation>> {
        match self.cache_repository.get(&Self::key(discord_user_id)).await {
            None => Ok(vec![]),
            Some(raw) => {
                serde_json::from_str(&raw).map_err(|_| StateServiceError::CacheError)
            }
        }
    }
}
