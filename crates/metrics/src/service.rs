use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use zako3_states::CacheRepositoryRef;
use zako3_types::hq::{TapId, history::PlayAudioHistory};

use crate::error::Result;
use crate::history::{PgUseHistoryRepository, UseHistoryRepository};
use crate::redis_metrics::TapRedisMetrics;

#[derive(Debug, Clone)]
pub struct TapMetricsRow {
    pub time: DateTime<Utc>,
    pub total_uses: i64,
    pub cache_hits: i64,
    pub active_now: i64,
    pub unique_users: i64,
}

#[derive(Clone)]
pub struct TapMetricsService {
    pub redis: TapRedisMetrics,
    pool: PgPool,
    history_repo: PgUseHistoryRepository,
}

impl TapMetricsService {
    pub fn new(redis: CacheRepositoryRef, pool: PgPool) -> Self {
        let history_repo = PgUseHistoryRepository::new(pool.clone());
        Self {
            redis: TapRedisMetrics::new(redis),
            pool,
            history_repo,
        }
    }

    pub async fn register_tap(&self, tap_id: TapId) -> Result<()> {
        self.redis.register_tap(tap_id).await
    }
    pub async fn acc_uptime(&self, tap_id: TapId, secs: i64) -> Result<()> {
        self.redis.acc_uptime(tap_id, secs).await
    }
    pub async fn get_uptime_secs(&self, tap_id: TapId) -> Result<u64> {
        self.redis.get_uptime_secs(tap_id).await
    }
    pub async fn get_known_taps(&self) -> Result<Vec<TapId>> {
        self.redis.get_known_taps().await
    }
    pub async fn get_unique_users_count(&self, tap_id: TapId) -> Result<u64> {
        self.redis.get_unique_users_count(tap_id).await
    }
    pub async fn get_global_unique_users(&self) -> Result<u64> {
        self.redis.get_global_unique_users().await
    }
    pub async fn incr_delta_total_uses(&self, tap_id: &TapId) -> Result<()> {
        self.redis.incr_delta_total_uses(tap_id).await
    }
    pub async fn incr_delta_cache_hits(&self, tap_id: &TapId) -> Result<()> {
        self.redis.incr_delta_cache_hits(tap_id).await
    }
    pub async fn drain_delta(&self, tap_id: &TapId) -> Result<(i64, i64)> {
        self.redis.drain_delta(tap_id).await
    }

    pub async fn get_latest_row(&self, tap_id: &TapId) -> Result<Option<TapMetricsRow>> {
        let row = sqlx::query(
            "SELECT time, total_uses, cache_hits, active_now, unique_users \
             FROM tap_metrics WHERE tap_id = $1 ORDER BY time DESC LIMIT 1",
        )
        .bind(&tap_id.0)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| TapMetricsRow {
            time: r.get("time"),
            total_uses: r.get("total_uses"),
            cache_hits: r.get("cache_hits"),
            active_now: r.get("active_now"),
            unique_users: r.get("unique_users"),
        }))
    }

    pub async fn get_latest_total_uses(&self, tap_id: &TapId) -> Result<u64> {
        Ok(self
            .get_latest_row(tap_id)
            .await?
            .map(|r| r.total_uses as u64)
            .unwrap_or(0))
    }

    pub async fn get_latest_cache_hits(&self, tap_id: &TapId) -> Result<u64> {
        Ok(self
            .get_latest_row(tap_id)
            .await?
            .map(|r| r.cache_hits as u64)
            .unwrap_or(0))
    }

    pub async fn get_time_series(
        &self,
        tap_id: &TapId,
        since: DateTime<Utc>,
    ) -> Result<Vec<TapMetricsRow>> {
        let rows = sqlx::query(
            "SELECT time, total_uses, cache_hits, active_now, unique_users \
             FROM tap_metrics WHERE tap_id = $1 AND time >= $2 ORDER BY time ASC",
        )
        .bind(&tap_id.0)
        .bind(since)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| TapMetricsRow {
                time: r.get("time"),
                total_uses: r.get("total_uses"),
                cache_hits: r.get("cache_hits"),
                active_now: r.get("active_now"),
                unique_users: r.get("unique_users"),
            })
            .collect())
    }

    pub async fn insert_metrics_row(
        &self,
        tap_id: &TapId,
        now: DateTime<Utc>,
        total_uses: i64,
        cache_hits: i64,
        active_now: i64,
        unique_users: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO tap_metrics (time, tap_id, total_uses, active_now, cache_hits, unique_users) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(now)
        .bind(&tap_id.0)
        .bind(total_uses)
        .bind(active_now)
        .bind(cache_hits)
        .bind(unique_users)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Full sync for one tap: drain delta -> fetch last DB row -> accumulate -> insert.
    pub async fn sync_tap(
        &self,
        tap_id: &TapId,
        now: DateTime<Utc>,
        active_now: i64,
        unique_users: i64,
    ) -> Result<()> {
        let (delta_total, delta_cache) = self.drain_delta(tap_id).await?;
        let last = self.get_latest_row(tap_id).await?;
        let last_total = last.as_ref().map(|r| r.total_uses).unwrap_or(0);
        let last_cache = last.as_ref().map(|r| r.cache_hits).unwrap_or(0);
        self.insert_metrics_row(
            tap_id,
            now,
            last_total + delta_total,
            last_cache + delta_cache,
            active_now,
            unique_users,
        )
        .await
    }

    pub async fn insert_history(&self, entry: &PlayAudioHistory) -> Result<()> {
        self.history_repo.insert(entry).await
    }
}
