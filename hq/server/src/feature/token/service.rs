use async_trait::async_trait;
use mockall::automock;

use crate::{
    feature::{
        auth::{
            error::AuthError,
            jwt::{check_refresh_token, sign_jwt},
            types::JwtPair,
        },
        config::ConfigRepository,
        service::{Service, ServiceRepository},
        token::repository::TokenRepository,
    },
    util::{
        error::{AppError, AppResult},
        snowflake::LazySnowflake,
    },
};

#[automock]
#[async_trait]
pub trait TokenService {
    async fn refresh_user_token(&self, refresh_token: &str) -> AppResult<JwtPair>;

    async fn issue_token(&self, user_id: LazySnowflake) -> AppResult<JwtPair>;

    // TODO revoke
}

#[async_trait]
impl<S> TokenService for Service<S>
where
    S: ServiceRepository,
{
    async fn issue_token(&self, user_id: LazySnowflake) -> AppResult<JwtPair> {
        let jwt_config = self.config_repo.jwt_config();

        let sign_result = sign_jwt(jwt_config.clone(), user_id)?;

        self.token_repo
            .add_refresh_token_user(
                sign_result.refresh_token_id,
                user_id,
                jwt_config.refresh_token_ttl,
            )
            .await?;

        Ok(sign_result.pair)
    }

    async fn refresh_user_token(&self, refresh_token: &str) -> AppResult<JwtPair> {
        let jwt_config = self.config_repo.jwt_config();

        let given_refresh_token =
            check_refresh_token(jwt_config.clone(), refresh_token.to_string())?;

        let user_id = self
            .token_repo
            .get_refresh_token_user(given_refresh_token.refresh_token_id)
            .await?;

        if let Some(user_id) = user_id {
            if given_refresh_token.user_id == user_id {
                // 1
                let sign_result = sign_jwt(jwt_config.clone(), user_id)?;

                self.token_repo
                    .add_refresh_token_user(
                        sign_result.refresh_token_id,
                        user_id,
                        jwt_config.refresh_token_ttl,
                    )
                    .await?;

                // 2
                self.token_repo
                    .delete_refresh_token_user(given_refresh_token.refresh_token_id)
                    .await?;

                // 3
                Ok(sign_result.pair)
            } else {
                Err(AppError::Auth(AuthError::InsufficientPrivileges))
            }
        } else {
            Err(AppError::Auth(AuthError::InsufficientPrivileges))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        feature::{
            auth::{jwt::sign_jwt, types::JwtConfig},
            service::{MockServiceRepository, Service},
            token::service::TokenService,
        },
        util::snowflake::LazySnowflake,
    };

    pub fn modify_service_repository_token(
        user_id: LazySnowflake,
        config: JwtConfig,
    ) -> impl Fn(Service<MockServiceRepository>) -> Service<MockServiceRepository> {
        move |mut s| {
            s.token_repo
                .expect_add_refresh_token_user()
                .returning(|_, _, _| Ok(()));
            s.token_repo
                .expect_delete_refresh_token_user()
                .returning(|_| Ok(()));
            s.token_repo
                .expect_get_refresh_token_user()
                .returning(move |_| Ok(Some(user_id)));

            let config = config.clone();
            s.config_repo
                .expect_jwt_config()
                .returning(move || config.clone());
            s
        }
    }

    #[tokio::test]
    async fn refresh_user_token_success() {
        let config = JwtConfig::default_testing();

        let user_id = LazySnowflake::from(1234);

        let service = MockServiceRepository::modified_service(modify_service_repository_token(
            user_id, config,
        ));

        let config = JwtConfig::default_testing();

        let r = sign_jwt(config, user_id).unwrap();

        let rs = service.refresh_user_token(&r.pair.refresh_token).await;

        assert!(rs.is_ok());
    }

    #[tokio::test]
    async fn issue_token_success() {
        let config = JwtConfig::default_testing();

        let user_id = LazySnowflake::from(1234);

        let service = MockServiceRepository::modified_service(|mut s| {
            s.token_repo
                .expect_add_refresh_token_user()
                .returning(|_, _, _| Ok(()));
            s.token_repo
                .expect_get_refresh_token_user()
                .returning(move |_| Ok(Some(user_id)));

            let config = config.clone();
            s.config_repo
                .expect_jwt_config()
                .returning(move || config.clone());
            s
        });

        let rs = service.issue_token(user_id).await;

        assert!(rs.is_ok());
    }
}
