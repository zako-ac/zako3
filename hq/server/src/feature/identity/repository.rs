use async_trait::async_trait;
use mockall::automock;

use crate::{
    feature::identity::{entity::Identity, error::IdentityResult},
    util::{permission::PermissionFlags, snowflake::LazySnowflake},
};

#[derive(Clone, Debug, Default)]
pub struct UpdateIdentity {
    pub name: Option<Option<String>>,
    pub permissions: Option<PermissionFlags>,
}

#[automock]
#[async_trait]
pub trait IdentityRepository {
    async fn find_identity(&self, id: LazySnowflake) -> IdentityResult<Option<Identity>>;

    async fn create_identity(&self, data: &Identity) -> IdentityResult<()>;

    async fn update_identity(&self, id: LazySnowflake, data: &UpdateIdentity)
    -> IdentityResult<()>;

    async fn delete_identity(&self, id: LazySnowflake) -> IdentityResult<()>;
}
