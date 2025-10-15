use std::time::Duration;

use async_trait::async_trait;
use redis::AsyncCommands;

use crate::{
    feature::token::repository::TokenRepository,
    infrastructure::redis::RedisDb,
    util::{
        error::{AppError, AppResult},
        snowflake::LazySnowflake,
    },
};

#[async_trait]
impl TokenRepository for RedisDb {
    async fn add_refresh_token_user(
        &self,
        token_id: LazySnowflake,
        user_id: LazySnowflake,
        ttl: Duration,
    ) -> AppResult<()> {
        let key = make_key(token_id);

        let _: () = self
            .connection_manager()
            .set_ex(key, user_id.to_string(), ttl.as_secs())
            .await?;

        Ok(())
    }

    async fn get_refresh_token_user(
        &self,
        token_id: LazySnowflake,
    ) -> AppResult<Option<LazySnowflake>> {
        let key = make_key(token_id);

        let user_id_str: Option<String> = self.connection_manager().get(key).await?;

        if let Some(user_id_str) = user_id_str {
            let user_id = user_id_str
                .parse::<u64>()
                .map_err(|_| AppError::Unknown(format!("expected number, got {}", user_id_str)))?;

            Ok(Some(LazySnowflake::from(user_id)))
        } else {
            Ok(None)
        }
    }

    async fn delete_refresh_token_user(&self, token_id: LazySnowflake) -> AppResult<()> {
        let key = make_key(token_id);

        let _: () = self.connection_manager().del(key).await?;

        Ok(())
    }
}

fn make_key(token_id: LazySnowflake) -> String {
    format!("refresh_token:{}", *token_id)
}
