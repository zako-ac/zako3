use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;
use zako3_types::{OnlineTapState, OnlineTapStates, TapName, hq::TapId};

#[derive(Error, Debug)]
pub enum StateServiceError {
    #[error("Cache error")]
    CacheError,
    #[cfg(feature = "redis")]
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),
}

type Result<T> = std::result::Result<T, StateServiceError>;

#[async_trait]
pub trait CacheRepository: Send + Sync {
    async fn get(&self, key: &str) -> Option<String>;
    async fn set(&self, key: &str, value: &str);
}

pub type CacheRepositoryRef = Arc<dyn CacheRepository>;

#[derive(Clone)]
pub struct TapHubStateService {
    pub cache_repository: CacheRepositoryRef,
}

impl TapHubStateService {
    pub fn new(cache_repository: CacheRepositoryRef) -> Self {
        Self { cache_repository }
    }

    pub async fn get_tap_id_by_name(&self, tap_name: &TapName) -> Result<Option<TapId>> {
        let name_key = format!("tap_name:{}", tap_name.0);
        let id_str = self.cache_repository.get(&name_key).await;
        match id_str {
            Some(id) => {
                if let Ok(uuid) = Uuid::parse_str(&id) {
                    Ok(Some(TapId(uuid)))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    pub async fn get_tap_states(&self, tap_id: &TapId) -> Result<OnlineTapStates> {
        let key = format!("tap:{}", tap_id.0);

        let state_str = match self.cache_repository.get(&key).await {
            Some(state) => state,
            None => return Ok(vec![]),
        };

        let state: OnlineTapStates =
            serde_json::from_str(&state_str).map_err(|_| StateServiceError::CacheError)?;

        Ok(state)
    }

    pub async fn set_tap_states(&self, tap_id: &TapId, state: &OnlineTapStates) -> Result<()> {
        let key = format!("tap:{}", tap_id.0);

        let state_str = serde_json::to_string(state).map_err(|_| StateServiceError::CacheError)?;
        self.cache_repository.set(&key, &state_str).await;
        Ok(())
    }

    pub async fn set_connection_state(&self, state: OnlineTapState) -> Result<()> {
        let tap_id = state.tap_id.clone();

        let name_key = format!("tap_name:{}", state.tap_name.0);
        self.cache_repository
            .set(&name_key, &tap_id.0.to_string())
            .await;

        let mut states = self.get_tap_states(&state.tap_id).await?;

        states.retain(|s| !(s.tap_id == state.tap_id && s.connection_id == state.connection_id));
        states.push(state);

        self.set_tap_states(&tap_id, &states).await?;

        Ok(())
    }

    pub async fn get_connection_state(
        &self,
        state: OnlineTapState,
    ) -> Result<Option<OnlineTapState>> {
        let states = self.get_tap_states(&state.tap_id).await?;
        let found = states
            .into_iter()
            .find(|s| s.tap_id == state.tap_id && s.connection_id == state.connection_id);

        Ok(found)
    }

    pub async fn remove_connection_state(&self, tap_id: &TapId, connection_id: u64) -> Result<()> {
        let mut states = self.get_tap_states(tap_id).await?;
        states.retain(|s| !(&s.tap_id == tap_id && s.connection_id == connection_id));
        self.set_tap_states(tap_id, &states).await?;

        Ok(())
    }
}

#[cfg(feature = "redis")]
#[derive(Clone)]
pub struct RedisCacheRepository {
    client: redis::aio::ConnectionManager,
}

#[cfg(feature = "redis")]
impl RedisCacheRepository {
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let connection_manager = client.get_connection_manager().await?;
        Ok(Self {
            client: connection_manager,
        })
    }
}

#[cfg(feature = "redis")]
#[async_trait]
impl CacheRepository for RedisCacheRepository {
    async fn get(&self, key: &str) -> Option<String> {
        use redis::AsyncCommands;
        let mut conn = self.client.clone();
        conn.get(key).await.ok()
    }

    async fn set(&self, key: &str, value: &str) {
        use redis::AsyncCommands;
        let mut conn = self.client.clone();
        let _: redis::RedisResult<()> = conn.set(key, value).await;
    }
}
