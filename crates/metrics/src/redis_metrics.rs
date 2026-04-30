use zako3_states::CacheRepositoryRef;
use zako3_types::hq::TapId;
use crate::error::Result;

#[derive(Clone)]
pub struct TapRedisMetrics {
    redis: CacheRepositoryRef,
}

impl TapRedisMetrics {
    pub fn new(redis: CacheRepositoryRef) -> Self {
        Self { redis }
    }

    fn uptime_key(&self, tap_id: &TapId) -> String {
        format!("metrics:{}:uptime_secs", tap_id.0)
    }

    fn unique_users_key(&self, tap_id: &TapId) -> String {
        format!("metrics:{}:unique_users", tap_id.0)
    }

    fn delta_key(&self, tap_id: &TapId) -> String {
        format!("delta_metrics:{}", tap_id.0)
    }

    pub async fn register_tap(&self, tap_id: TapId) -> Result<()> {
        self.redis.sadd("metrics:known_taps", &tap_id.0).await?;
        Ok(())
    }

    pub async fn acc_uptime(&self, tap_id: TapId, secs: i64) -> Result<()> {
        let key = self.uptime_key(&tap_id);
        self.redis.incrby(&key, secs).await?;
        Ok(())
    }

    pub async fn get_uptime_secs(&self, tap_id: TapId) -> Result<u64> {
        let key = self.uptime_key(&tap_id);
        Ok(self
            .redis
            .get(&key)
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(0))
    }

    pub async fn get_known_taps(&self) -> Result<Vec<TapId>> {
        let members = self.redis.smembers("metrics:known_taps").await?;
        Ok(members.into_iter().map(TapId).collect())
    }

    pub async fn get_unique_users_count(&self, tap_id: TapId) -> Result<u64> {
        let key = self.unique_users_key(&tap_id);
        self.redis.pfcount(&key).await.map_err(Into::into)
    }

    pub async fn get_global_unique_users(&self) -> Result<u64> {
        let taps = self.get_known_taps().await?;
        if taps.is_empty() {
            return Ok(0);
        }
        let keys: Vec<String> = taps
            .iter()
            .map(|id| format!("metrics:{}:unique_users", id.0))
            .collect();
        self.redis.pfcount_multi(&keys).await.map_err(Into::into)
    }

    pub async fn incr_delta_total_uses(&self, tap_id: &TapId) -> Result<()> {
        let key = self.delta_key(tap_id);
        self.redis.hincrby(&key, "total_uses", 1).await?;
        Ok(())
    }

    pub async fn incr_delta_cache_hits(&self, tap_id: &TapId) -> Result<()> {
        let key = self.delta_key(tap_id);
        self.redis.hincrby(&key, "cache_hits", 1).await?;
        Ok(())
    }

    /// Returns (delta_total_uses, delta_cache_hits), atomically clearing the hash.
    pub async fn drain_delta(&self, tap_id: &TapId) -> Result<(i64, i64)> {
        let key = self.delta_key(tap_id);
        let fields = self.redis.hgetall(&key).await.unwrap_or_default();
        let _ = self.redis.hdel_key(&key).await;

        let total = fields
            .iter()
            .find(|(k, _)| k == "total_uses")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(0i64);
        let cache = fields
            .iter()
            .find(|(k, _)| k == "cache_hits")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(0i64);
        Ok((total, cache))
    }
}
