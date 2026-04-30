use crate::cache_repo::CacheRepositoryRef;
use crate::error::{Result, StateServiceError};

#[derive(Clone)]
pub struct IntendedVoiceChannelService {
    cache_repository: CacheRepositoryRef,
}

impl IntendedVoiceChannelService {
    pub fn new(cache_repository: CacheRepositoryRef) -> Self {
        Self { cache_repository }
    }

    fn key(guild_id: u64) -> String {
        format!("intended_vcs:{}", guild_id)
    }

    async fn get_all(&self, guild_id: u64) -> Result<Vec<u64>> {
        match self.cache_repository.get(&Self::key(guild_id)).await {
            None => Ok(vec![]),
            Some(raw) => serde_json::from_str(&raw).map_err(|_| StateServiceError::CacheError),
        }
    }

    pub async fn add(&self, guild_id: u64, channel_id: u64) -> Result<()> {
        let mut channels = self.get_all(guild_id).await?;
        if !channels.contains(&channel_id) {
            channels.push(channel_id);
            let raw = serde_json::to_string(&channels).map_err(|_| StateServiceError::CacheError)?;
            self.cache_repository.set(&Self::key(guild_id), &raw).await;
        }
        Ok(())
    }

    pub async fn remove(&self, guild_id: u64, channel_id: u64) -> Result<()> {
        let mut channels = self.get_all(guild_id).await?;
        channels.retain(|&c| c != channel_id);
        if channels.is_empty() {
            self.cache_repository.del(&Self::key(guild_id)).await;
        } else {
            let raw = serde_json::to_string(&channels).map_err(|_| StateServiceError::CacheError)?;
            self.cache_repository.set(&Self::key(guild_id), &raw).await;
        }
        Ok(())
    }

    pub async fn contains(&self, guild_id: u64, channel_id: u64) -> Result<bool> {
        Ok(self.get_all(guild_id).await?.contains(&channel_id))
    }
}
