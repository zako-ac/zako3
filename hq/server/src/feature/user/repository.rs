use async_trait::async_trait;
use mockall::automock;

use crate::{
    feature::user::User,
    util::{error::AppResult, permission::PermissionFlags, snowflake::LazySnowflake},
};

#[derive(Clone, Debug, Default)]
pub struct UpdateUser {
    pub name: Option<String>,
    pub permissions: Option<PermissionFlags>,
}

#[automock]
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_user(&self, id: LazySnowflake) -> AppResult<Option<User>>;

    async fn create_user(&self, data: User) -> AppResult<()>;

    async fn update_user(&self, id: LazySnowflake, data: UpdateUser) -> AppResult<()>;

    async fn delete_user(&self, id: LazySnowflake) -> AppResult<()>;
}
