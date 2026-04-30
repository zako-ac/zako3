use zako3_types::hq::TapId;

use crate::cache_repo::CacheRepositoryRef;
use crate::error::Result;

pub enum TapMetricKey {
    TotalUses,
    CacheHits,
    UniqueUsers,
    UptimeSecs,
}

impl TapMetricKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TotalUses => "total_uses",
            Self::CacheHits => "cache_hits",
            Self::UniqueUsers => "unique_users",
            Self::UptimeSecs => "uptime_secs",
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

    pub async fn get_unique_users_count(&self, tap_id: TapId) -> Result<u64> {
        let key = self.get_key(tap_id, TapMetricKey::UniqueUsers);
        self.cache_repository.pfcount(&key).await
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
        self.cache_repository.pfcount_multi(&keys).await
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

    pub async fn acc_uptime(&self, tap_id: TapId, secs: i64) -> Result<()> {
        let key = self.get_key(tap_id, TapMetricKey::UptimeSecs);
        self.cache_repository.incrby(&key, secs).await?;
        Ok(())
    }

    pub async fn get_known_taps(&self) -> Result<Vec<TapId>> {
        let key = "metrics:known_taps";
        let members = self.cache_repository.smembers(key).await?;
        Ok(members.into_iter().map(TapId).collect())
    }
}
