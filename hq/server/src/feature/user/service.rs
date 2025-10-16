use async_trait::async_trait;
use mockall::automock;

use crate::{
    feature::{
        service::{Service, ServiceRepository},
        user::{
            User,
            repository::{UpdateUser, UserRepository},
            types::{CreateUser, UpdateUserInfo, UpdateUserPermissions},
        },
    },
    util::{
        error::AppResult,
        snowflake::{LazySnowflake, Snowflake},
    },
};

#[automock]
#[async_trait]
pub trait UserService {
    async fn create_user(&self, data: CreateUser) -> AppResult<User>;
    async fn get_user(&self, user_id: LazySnowflake) -> AppResult<Option<User>>;
    async fn update_user_information(
        &self,
        user_id: LazySnowflake,
        data: UpdateUserInfo,
    ) -> AppResult<()>;
    async fn update_user_permissions(
        &self,
        user_id: LazySnowflake,
        data: UpdateUserPermissions,
    ) -> AppResult<()>;
    async fn delete_user(&self, user_id: LazySnowflake) -> AppResult<()>;
}

#[async_trait]
impl<S> UserService for Service<S>
where
    S: ServiceRepository,
{
    async fn create_user(&self, data: CreateUser) -> AppResult<User> {
        let user_id = Snowflake::new_now().as_lazy();

        let user = User {
            id: user_id,
            name: data.name,
            permissions: data.permissions,
        };

        self.user_repo.create_user(user.clone()).await?;

        Ok(user)
    }

    async fn get_user(&self, user_id: LazySnowflake) -> AppResult<Option<User>> {
        self.user_repo.find_user(user_id).await
    }

    async fn update_user_information(
        &self,
        user_id: LazySnowflake,
        data: UpdateUserInfo,
    ) -> AppResult<()> {
        self.user_repo
            .update_user(
                user_id,
                UpdateUser {
                    name: Some(data.name),
                    permissions: None,
                },
            )
            .await
    }

    async fn update_user_permissions(
        &self,
        user_id: LazySnowflake,
        data: UpdateUserPermissions,
    ) -> AppResult<()> {
        self.user_repo
            .update_user(
                user_id,
                UpdateUser {
                    name: None,
                    permissions: Some(data.permissions),
                },
            )
            .await
    }

    async fn delete_user(&self, user_id: LazySnowflake) -> AppResult<()> {
        self.user_repo.delete_user(user_id).await
    }
}
