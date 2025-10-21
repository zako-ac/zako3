use crate::{
    feature::user::{
        User,
        repository::{UpdateUser, UserRepository},
        types::{CreateUser, UpdateUserInfo, UpdateUserPermissions},
    },
    util::{
        error::AppResult,
        snowflake::{LazySnowflake, Snowflake},
    },
};

pub struct UserService<UR>
where
    UR: UserRepository,
{
    pub user_repo: UR,
}

impl<UR> UserService<UR>
where
    UR: UserRepository,
{
    pub async fn create_user(&self, data: CreateUser) -> AppResult<User> {
        let user_id = Snowflake::new_now().as_lazy();

        let user = User {
            id: user_id,
            name: data.name,
            permissions: data.permissions,
        };

        self.user_repo.create_user(user.clone()).await?;

        Ok(user)
    }

    pub async fn get_user(&self, user_id: LazySnowflake) -> AppResult<Option<User>> {
        self.user_repo.find_user(user_id).await
    }

    pub async fn update_user_information(
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

    pub async fn update_user_permissions(
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

    pub async fn delete_user(&self, user_id: LazySnowflake) -> AppResult<()> {
        self.user_repo.delete_user(user_id).await
    }
}
