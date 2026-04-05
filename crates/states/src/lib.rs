use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;
use zako3_types::{OnlineTapState, OnlineTapStates, TapName, hq::TapId, hq::UserId};

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
    async fn incr(&self, key: &str) -> Result<i64>;
    async fn decr(&self, key: &str) -> Result<i64>;
    async fn pfadd(&self, key: &str, element: &str) -> Result<()>;
    async fn pfcount(&self, key: &str) -> Result<u64>;
    async fn sadd(&self, key: &str, member: &str) -> Result<()>;
    async fn smembers(&self, key: &str) -> Result<Vec<String>>;
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
        Ok(id_str.map(TapId))
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
        self.cache_repository.set(&name_key, &tap_id.0).await;

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

pub enum TapMetricKey {
    TotalUses,
    ActiveNow,
    CacheHits,
    UniqueUsers,
}

impl TapMetricKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TotalUses => "total_uses",
            Self::ActiveNow => "active_now",
            Self::CacheHits => "cache_hits",
            Self::UniqueUsers => "unique_users",
        }
    }
}

#[derive(Clone)]
pub struct TapMetricsStateService {
    pub cache_repository: CacheRepositoryRef,
}

impl TapMetricsStateService {
    pub fn new(cache_repository: CacheRepositoryRef) -> Self {
        Self { cache_repository }
    }

    fn get_key(&self, tap_id: TapId, key: TapMetricKey) -> String {
        format!("metrics:{}:{}", tap_id.0, key.as_str())
    }

    pub async fn inc_total_uses(&self, tap_id: TapId) -> Result<()> {
        let key = self.get_key(tap_id, TapMetricKey::TotalUses);
        self.cache_repository.incr(&key).await?;
        Ok(())
    }

    pub async fn inc_active_now(&self, tap_id: TapId) -> Result<()> {
        let key = self.get_key(tap_id, TapMetricKey::ActiveNow);
        self.cache_repository.incr(&key).await?;
        Ok(())
    }

    pub async fn dec_active_now(&self, tap_id: TapId) -> Result<()> {
        let key = self.get_key(tap_id, TapMetricKey::ActiveNow);
        self.cache_repository.decr(&key).await?;
        Ok(())
    }

    pub async fn inc_cache_hits(&self, tap_id: TapId) -> Result<()> {
        let key = self.get_key(tap_id, TapMetricKey::CacheHits);
        self.cache_repository.incr(&key).await?;
        Ok(())
    }

    pub async fn record_unique_user(&self, tap_id: TapId, user_id: UserId) -> Result<()> {
        let key = self.get_key(tap_id, TapMetricKey::UniqueUsers);
        self.cache_repository.pfadd(&key, &user_id.0).await?;
        Ok(())
    }

    pub async fn get_unique_users_count(&self, tap_id: TapId) -> Result<u64> {
        let key = self.get_key(tap_id, TapMetricKey::UniqueUsers);
        self.cache_repository.pfcount(&key).await
    }

    pub async fn get_metric(&self, tap_id: TapId, key: TapMetricKey) -> Result<u64> {
        let redis_key = self.get_key(tap_id, key);
        let val = self.cache_repository.get(&redis_key).await;
        match val {
            Some(v) => Ok(v.parse().unwrap_or(0)),
            None => Ok(0),
        }
    }

    pub async fn register_tap(&self, tap_id: TapId) -> Result<()> {
        let key = "metrics:known_taps";
        self.cache_repository
            .sadd(key, &tap_id.0.to_string())
            .await?;
        Ok(())
    }

    pub async fn get_known_taps(&self) -> Result<Vec<TapId>> {
        let key = "metrics:known_taps";
        let members = self.cache_repository.smembers(key).await?;
        Ok(members.into_iter().map(TapId).collect())
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

    async fn incr(&self, key: &str) -> Result<i64> {
        use redis::AsyncCommands;
        let mut conn = self.client.clone();
        let val: i64 = conn.incr(key, 1).await?;
        Ok(val)
    }

    async fn decr(&self, key: &str) -> Result<i64> {
        use redis::AsyncCommands;
        let mut conn = self.client.clone();
        let val: i64 = conn.decr(key, 1).await?;
        Ok(val)
    }

    async fn pfadd(&self, key: &str, element: &str) -> Result<()> {
        use redis::AsyncCommands;
        let mut conn = self.client.clone();
        let _: () = conn.pfadd(key, element).await?;
        Ok(())
    }

    async fn pfcount(&self, key: &str) -> Result<u64> {
        use redis::AsyncCommands;
        let mut conn = self.client.clone();
        let val: u64 = conn.pfcount(key).await?;
        Ok(val)
    }

    async fn sadd(&self, key: &str, member: &str) -> Result<()> {
        use redis::AsyncCommands;
        let mut conn = self.client.clone();
        let _: () = conn.sadd(key, member).await?;
        Ok(())
    }

    async fn smembers(&self, key: &str) -> Result<Vec<String>> {
        use redis::AsyncCommands;
        let mut conn = self.client.clone();
        let val: Vec<String> = conn.smembers(key).await?;
        Ok(val)
    }
}
