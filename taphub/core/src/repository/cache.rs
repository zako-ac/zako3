use std::sync::Arc;

use async_trait::async_trait;

#[async_trait]
pub trait CacheRepository: Send + Sync {
    async fn get(&self, key: &str) -> Option<String>;
    async fn set(&self, key: &str, value: &str);
}

pub type CacheRepositoryRef = Arc<dyn CacheRepository>;
