use async_trait::async_trait;
use mockall::automock;

use crate::{
    feature::user::{User, error::UserResult},
    util::{permission::PermissionFlags, snowflake::LazySnowflake},
};

#[derive(Clone, Debug, Default)]
pub struct UpdateUser {
    pub name: Option<Option<String>>,
    pub permissions: Option<PermissionFlags>,
}

#[automock]
#[async_trait]
pub trait UserRepository {
    async fn find_user(&self, id: LazySnowflake) -> UserResult<Option<User>>;

    async fn create_user(&self, data: &User) -> UserResult<()>;

    async fn update_user(&self, id: LazySnowflake, data: &UpdateUser) -> UserResult<()>;

    async fn delete_user(&self, id: LazySnowflake) -> UserResult<()>;
}
