use mockall::automock;

use crate::{
    feature::identity::{entity::Identity, error::IdentityResult},
    util::snowflake::LazySnowflake,
};

#[derive(Clone, Debug)]
pub struct UpdateIdentity {
    pub name: Option<String>,
}

#[automock]
pub trait IdentityRepository {
    async fn find_identity(&self, id: LazySnowflake) -> IdentityResult<Option<Identity>>;

    async fn create_identity(&self, data: &Identity) -> IdentityResult<()>;

    async fn update_identity(&self, id: LazySnowflake, data: &UpdateIdentity)
    -> IdentityResult<()>;

    async fn delete_identity(&self, id: LazySnowflake) -> IdentityResult<()>;
}
