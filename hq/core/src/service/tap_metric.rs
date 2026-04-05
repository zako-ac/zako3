use crate::repo::TapMetricRepository;
use anyhow::Result;
use std::sync::Arc;

pub struct TapMetricService {
    repo: Arc<TapMetricRepository>,
}

impl TapMetricService {
    pub fn new(repo: Arc<TapMetricRepository>) -> Self {
        Self { repo }
    }

    pub async fn record_request(&self, tap_id: u64) -> Result<()> {
        self.repo.record_metric(tap_id, "request").await
    }

    pub async fn record_cache_hit(&self, tap_id: u64) -> Result<()> {
        self.repo.record_metric(tap_id, "cache_hit").await
    }

    pub async fn record_error(&self, tap_id: u64) -> Result<()> {
        self.repo.record_metric(tap_id, "error").await
    }

    pub async fn get_total_uses(&self, tap_id: u64) -> Result<i64> {
        self.repo.get_total_uses(tap_id).await
    }
}
