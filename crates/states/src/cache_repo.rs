use async_trait::async_trait;
use std::sync::Arc;

use crate::error::Result;

#[async_trait]
pub trait CacheRepository: Send + Sync {
    async fn get(&self, key: &str) -> Option<String>;
    async fn set(&self, key: &str, value: &str);
    async fn set_ex(&self, key: &str, value: &str, ttl_secs: u64);
    async fn del(&self, key: &str);
    async fn incr(&self, key: &str) -> Result<i64>;
    async fn decr(&self, key: &str) -> Result<i64>;
    async fn incrby(&self, key: &str, amount: i64) -> Result<i64>;
    async fn pfadd(&self, key: &str, element: &str) -> Result<()>;
    async fn pfcount(&self, key: &str) -> Result<u64>;
    async fn pfcount_multi(&self, keys: &[String]) -> Result<u64>;
    async fn sadd(&self, key: &str, member: &str) -> Result<()>;
    async fn smembers(&self, key: &str) -> Result<Vec<String>>;
    async fn hgetall(&self, key: &str) -> Result<Vec<(String, String)>>;
    async fn hincrby(&self, key: &str, field: &str, amount: i64) -> Result<i64>;
    async fn hdel_key(&self, key: &str) -> Result<()>;
}

pub type CacheRepositoryRef = Arc<dyn CacheRepository>;

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

    async fn set_ex(&self, key: &str, value: &str, ttl_secs: u64) {
        use redis::AsyncCommands;
        let mut conn = self.client.clone();
        let _: redis::RedisResult<()> = conn.set_ex(key, value, ttl_secs).await;
    }

    async fn del(&self, key: &str) {
        use redis::AsyncCommands;
        let mut conn = self.client.clone();
        let _: redis::RedisResult<()> = conn.del(key).await;
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

    async fn incrby(&self, key: &str, amount: i64) -> Result<i64> {
        use redis::AsyncCommands;
        let mut conn = self.client.clone();
        let val: i64 = conn.incr(key, amount).await?;
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

    async fn pfcount_multi(&self, keys: &[String]) -> Result<u64> {
        use redis::AsyncCommands;
        let mut conn = self.client.clone();
        let val: u64 = conn.pfcount(keys).await?;
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

    async fn hgetall(&self, key: &str) -> Result<Vec<(String, String)>> {
        use redis::AsyncCommands;
        let mut conn = self.client.clone();
        let val: Vec<(String, String)> = conn.hgetall(key).await?;
        Ok(val)
    }

    async fn hincrby(&self, key: &str, field: &str, amount: i64) -> Result<i64> {
        use redis::AsyncCommands;
        let mut conn = self.client.clone();
        let val: i64 = conn.hincr(key, field, amount).await?;
        Ok(val)
    }

    async fn hdel_key(&self, key: &str) -> Result<()> {
        use redis::AsyncCommands;
        let mut conn = self.client.clone();
        let _: redis::RedisResult<()> = conn.del(key).await;
        Ok(())
    }
}
