use std::time::Duration;

use async_trait::async_trait;

use crate::util::{error::AppResult, snowflake::LazySnowflake};

#[async_trait]
pub trait TokenRepository {
    async fn add_refresh_token_user(
        &self,
        token_id: LazySnowflake,
        user_id: LazySnowflake,
        ttl: Duration,
    ) -> AppResult<()>;

    async fn get_refresh_token_user(
        &self,
        token_id: LazySnowflake,
    ) -> AppResult<Option<LazySnowflake>>;

    async fn delete_refresh_token_user(&self, token_id: LazySnowflake) -> AppResult<()>;
}
