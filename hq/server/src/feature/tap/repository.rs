use async_trait::async_trait;
use mockall::automock;

use crate::{
    feature::tap::{Tap, TapName},
    util::{error::AppResult, snowflake::LazySnowflake},
};

#[derive(Clone, Debug, Default)]
pub struct UpdateTap {
    pub name: Option<TapName>,
}

#[automock]
#[async_trait]
pub trait TapRepository: Send + Sync {
    async fn find_tap(&self, id: LazySnowflake) -> AppResult<Option<Tap>>;

    async fn create_tap(&self, data: Tap) -> AppResult<()>;

    async fn update_tap(&self, id: LazySnowflake, data: UpdateTap) -> AppResult<()>;

    async fn delete_tap(&self, id: LazySnowflake) -> AppResult<()>;
}
