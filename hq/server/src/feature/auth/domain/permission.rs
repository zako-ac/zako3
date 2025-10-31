use tracing::instrument;

use crate::{
    core::app::AppState,
    feature::{
        auth::domain::{error::AuthError, jwt::check_access_token},
        config::model::JwtConfig,
        user::{User, repository::UserRepository, service::UserService},
    },
    util::{error::AppResult, permission::PermissionFlags, snowflake::LazySnowflake},
};

#[derive(Clone, Debug)]
pub enum OwnedPermission {
    Public,
    OwnerOnly(LazySnowflake),
    AdminOnly,
}

fn check_logic_non_public(permission: OwnedPermission, user: User) -> AppResult<()> {
    match permission {
        OwnedPermission::Public => unreachable!(),
        OwnedPermission::AdminOnly => {
            if user.permissions.contains(PermissionFlags::Admin) {
                Ok(())
            } else {
                Err(AuthError::InsufficientPrivileges.into())
            }
        }
        OwnedPermission::OwnerOnly(owner_id) => {
            if user.permissions.contains(PermissionFlags::BaseUser) {
                if user.id == owner_id || user.permissions.contains(PermissionFlags::Admin) {
                    Ok(())
                } else {
                    Err(AuthError::InsufficientPrivileges.into())
                }
            } else if user.permissions.contains(PermissionFlags::Admin) {
                Ok(())
            } else {
                Err(AuthError::NotAllowedService.into())
            }
        }
    }
}

pub async fn check_permission(
    permission: OwnedPermission,
    access_token: String,
    app: &AppState,
) -> AppResult<()> {
    let app = app.clone();
    check_logic(
        permission,
        access_token,
        app.config.jwt,
        &app.service.user_service,
    )
    .await
}

#[instrument(skip_all)]
pub async fn check_logic(
    permission: OwnedPermission,
    access_token: String,
    config: JwtConfig,
    service: &UserService<impl UserRepository>,
) -> AppResult<()> {
    match permission {
        OwnedPermission::Public => Ok(()),
        OwnedPermission::AdminOnly | OwnedPermission::OwnerOnly(_) => {
            let user_id = check_access_token(config, access_token)?;

            let user = service
                .get_user(user_id)
                .await?
                .ok_or(AuthError::UserNotExists)?;

            check_logic_non_public(permission, user)
        }
    }
}

#[cfg(test)]
mod tests {

    use mockall::predicate::eq;

    use crate::{
        feature::{
            auth::domain::{
                error::AuthError,
                jwt::tests::sign_jwt_testing,
                permission::{OwnedPermission, check_logic, check_logic_non_public},
            },
            config::model::JwtConfig,
            user::{User, repository::MockUserRepository, service::UserService},
        },
        util::{
            error::AppError,
            permission::PermissionFlags,
            snowflake::{LazySnowflake, Snowflake},
        },
    };

    #[tokio::test]
    async fn check_logic_admin_only_success() {
        let config = JwtConfig::default_testing();

        let user_id = LazySnowflake::from(12);

        let token = sign_jwt_testing(config.clone(), user_id);

        let mut user_repo = MockUserRepository::new();
        user_repo
            .expect_find_user()
            .with(eq(user_id))
            .returning(|_| {
                Ok(Some(User {
                    id: LazySnowflake::from(12),
                    name: "".to_string(),
                    permissions: PermissionFlags::Admin,
                }))
            });

        let service = UserService { user_repo };

        let r = check_logic(OwnedPermission::AdminOnly, token, config, &service).await;
        assert!(r.is_ok())
    }

    #[tokio::test]
    async fn check_logic_admin_only_fail() {
        let config = JwtConfig::default_testing();

        let user_id = LazySnowflake::from(12);

        let mut user_repo = MockUserRepository::new();
        user_repo
            .expect_find_user()
            .with(eq(user_id))
            .returning(|_| {
                Ok(Some(User {
                    id: LazySnowflake::from(12),
                    name: "".to_string(),
                    permissions: PermissionFlags::BaseUser,
                }))
            });
        let service = UserService { user_repo };

        let token = sign_jwt_testing(config.clone(), user_id);

        let r = check_logic(OwnedPermission::AdminOnly, token, config, &service).await;
        assert!(matches!(
            r,
            Err(AppError::Auth(AuthError::InsufficientPrivileges))
        ));
    }

    #[tokio::test]
    async fn check_logic_user_invalid_fail() {
        let config = JwtConfig::default_testing();

        let mut user_repo = MockUserRepository::new();
        user_repo.expect_find_user().returning(|_| Ok(None));
        let service = UserService { user_repo };

        let user_id = LazySnowflake::from(13);

        let token = sign_jwt_testing(config.clone(), user_id);

        let r = check_logic(OwnedPermission::AdminOnly, token, config, &service).await;
        assert!(matches!(r, Err(AppError::Auth(AuthError::UserNotExists))));
    }

    #[test]
    fn check_logic_non_public_admin_success() {
        let r = check_logic_non_public(
            OwnedPermission::AdminOnly,
            User {
                id: Snowflake::new_now().as_lazy(),
                name: "asdf".to_string(),
                permissions: PermissionFlags::Admin,
            },
        );

        assert!(r.is_ok());
    }

    #[test]
    fn check_logic_non_public_admin_fail() {
        let r = check_logic_non_public(
            OwnedPermission::AdminOnly,
            User {
                id: Snowflake::new_now().as_lazy(),
                name: "asdf".to_string(),
                permissions: PermissionFlags::empty(),
            },
        );

        assert!(matches!(
            r,
            Err(AppError::Auth(AuthError::InsufficientPrivileges))
        ));
    }

    #[test]
    fn check_logic_non_public_owner_success_normal() {
        let r = check_logic_non_public(
            OwnedPermission::OwnerOnly(13.into()),
            User {
                id: 13.into(),
                name: "asdf".to_string(),
                permissions: PermissionFlags::BaseUser,
            },
        );

        assert!(r.is_ok());
    }

    #[test]
    fn check_logic_non_public_owner_success_admin() {
        let r = check_logic_non_public(
            OwnedPermission::OwnerOnly(13.into()),
            User {
                id: 14.into(),
                name: "asdf".to_string(),
                permissions: PermissionFlags::Admin,
            },
        );

        assert!(r.is_ok());
    }

    #[test]
    fn check_logic_non_public_owner_fail_not_service() {
        let r = check_logic_non_public(
            OwnedPermission::OwnerOnly(13.into()),
            User {
                id: 13.into(),
                name: "asdf".to_string(),
                permissions: PermissionFlags::empty(),
            },
        );

        assert!(matches!(
            r,
            Err(AppError::Auth(AuthError::NotAllowedService))
        ));
    }

    #[test]
    fn check_logic_non_public_owner_fail_different_user() {
        let r = check_logic_non_public(
            OwnedPermission::OwnerOnly(13.into()),
            User {
                id: 14.into(),
                name: "asdf".to_string(),
                permissions: PermissionFlags::BaseUser,
            },
        );

        assert!(matches!(
            r,
            Err(AppError::Auth(AuthError::InsufficientPrivileges))
        ));
    }
}
