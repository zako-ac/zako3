use std::sync::Arc;

use async_trait::async_trait;
use zako3_types::hq::Tap;

#[async_trait]
pub trait HqRepository: Send + Sync {
    async fn authenticate_tap(&self, token: &str) -> Option<Tap>;
    async fn get_tap(&self, tap_id: &str) -> Option<Tap>;
}

pub type HqRepositoryRef = Arc<dyn HqRepository>;
