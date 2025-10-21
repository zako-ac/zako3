use crate::{
    feature::{
        auth::domain::{
            error::AuthError,
            jwt::{check_refresh_token, sign_jwt},
            model::JwtPair,
        },
        config::model::{DebugConfig, JwtConfig},
        token::repository::TokenRepository,
    },
    util::{
        error::{AppError, AppResult},
        password::verify_password,
        snowflake::LazySnowflake,
    },
};

pub struct AuthService<TR>
where
    TR: TokenRepository,
{
    pub jwt_config: JwtConfig,
    pub debug_config: DebugConfig,
    pub token_repo: TR,
}

impl<TR> AuthService<TR>
where
    TR: TokenRepository,
{
    pub async fn test_login(&self, password: &str) -> AppResult<JwtPair> {
        let correct_password_hash = self
            .debug_config
            .debug_password_argon2
            .clone()
            .ok_or(AppError::Auth(AuthError::AttemptToLoginTestAccount))?;

        tracing::info!(event = "login", kind = "test_login");

        if verify_password(password, &correct_password_hash)? {
            // Testing user ID is 0
            let user_id = LazySnowflake::from(0);
            let pair = self.issue_token(user_id).await?;

            Ok(pair)
        } else {
            Err(AppError::Auth(AuthError::AttemptToLoginTestAccount))
        }
    }

    pub async fn issue_token(&self, user_id: LazySnowflake) -> AppResult<JwtPair> {
        let jwt_config = self.jwt_config.clone();

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

    pub async fn refresh_user_token(&self, refresh_token: &str) -> AppResult<JwtPair> {
        let jwt_config = self.jwt_config.clone();

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

    pub async fn revoke_refresh_token(
        token_repository: impl TokenRepository,
        refresh_token_id: LazySnowflake,
    ) -> AppResult<()> {
        token_repository
            .delete_refresh_token_user(refresh_token_id)
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        feature::{
            auth::{domain::jwt::sign_jwt, service::AuthService},
            config::model::JwtConfig,
            token::repository::MockTokenRepository,
        },
        util::snowflake::LazySnowflake,
    };

    fn mock_token_repository(user_id: LazySnowflake) -> MockTokenRepository {
        let mut token_repo = MockTokenRepository::new();
        token_repo
            .expect_add_refresh_token_user()
            .returning(|_, _, _| Ok(()));
        token_repo
            .expect_delete_refresh_token_user()
            .returning(|_| Ok(()));
        token_repo
            .expect_get_refresh_token_user()
            .returning(move |_| Ok(Some(user_id)));

        token_repo
    }

    fn mock_auth_service(
        jwt_config: JwtConfig,
        user_id: LazySnowflake,
    ) -> AuthService<MockTokenRepository> {
        let token_repo = mock_token_repository(user_id);
        AuthService {
            jwt_config,
            token_repo,
            debug_config: Default::default(),
        }
    }

    #[tokio::test]
    async fn refresh_user_token_success() {
        let user_id = LazySnowflake::from(1234);

        let jwt_config = JwtConfig::default_testing();
        let service = mock_auth_service(jwt_config.clone(), user_id);

        let r = sign_jwt(jwt_config, user_id).unwrap();

        let refreshed = service.refresh_user_token(&r.pair.refresh_token).await;

        assert!(refreshed.is_ok());
    }

    #[tokio::test]
    async fn issue_token_success() {
        let user_id = LazySnowflake::from(1234);

        let jwt_config = JwtConfig::default_testing();
        let service = mock_auth_service(jwt_config, user_id);

        let refreshed = service.issue_token(user_id).await;

        assert!(refreshed.is_ok());
    }
}
