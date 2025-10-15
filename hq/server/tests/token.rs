use std::time::Duration;

use zako3_hq_server::{
    feature::token::repository::TokenRepository, util::snowflake::LazySnowflake,
};

use crate::common::redis::create_redis_test;

pub mod common;

#[tokio::test]
async fn test_refresh_token_cache() {
    let (_guard, redis) = create_redis_test().await;

    {
        let user_id = LazySnowflake::from(1234);
        let token_id = LazySnowflake::from(5678);
        let ttl = Duration::from_secs(60);

        redis
            .add_refresh_token_user(token_id, user_id, ttl)
            .await
            .unwrap();

        let got_user_id = redis
            .get_refresh_token_user(token_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user_id, got_user_id);
    }

    {
        let got_user_id = redis
            .get_refresh_token_user(LazySnowflake::from(1))
            .await
            .unwrap();
        assert_eq!(got_user_id, None);
    }

    {
        let user_id = LazySnowflake::from(1234);
        let token_id = LazySnowflake::from(5678);
        let ttl = Duration::from_secs(60);

        redis
            .add_refresh_token_user(token_id, user_id, ttl)
            .await
            .unwrap();

        redis.delete_refresh_token_user(token_id).await.unwrap();

        let got_user_id = redis
            .get_refresh_token_user(LazySnowflake::from(1))
            .await
            .unwrap();
        assert_eq!(got_user_id, None);
    }
}
