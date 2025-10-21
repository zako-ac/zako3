use async_trait::async_trait;
use mockall::automock;

use crate::{
    feature::{
        auth::{error::AuthError, types::JwtPair},
        config::ConfigRepository,
        service::{Service, ServiceRepository},
        token::service::TokenService,
    },
    util::{
        error::{AppError, AppResult},
        password::verify_password,
        snowflake::LazySnowflake,
    },
};

#[automock]
#[async_trait]
pub trait AuthService {
    async fn test_login(&self, password: &str) -> AppResult<JwtPair>;
}

#[async_trait]
impl<S> AuthService for Service<S>
where
    S: ServiceRepository,
{
    async fn test_login(&self, password: &str) -> AppResult<JwtPair> {
        let correct_password_hash = self.config_repo.debug_password_argon2();

        if let Some(correct_password_hash) = correct_password_hash {
            tracing::info!(event = "login", kind = "test_login");

            if let Ok(true) = verify_password(password, &correct_password_hash) {
                // Testing user ID is 0
                let user_id = LazySnowflake::from(0);
                let pair = self.issue_token(user_id).await?;
                Ok(pair)
            } else {
                Err(AppError::Auth(AuthError::InsufficientPrivileges)).into()
            }
        } else {
            Err(AppError::Auth(AuthError::InsufficientPrivileges)).into()
        }
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_login_success() {
        let config = JwtConfig::default_testing();

        let user_id = LazySnowflake::from(1234);

        let service = MockServiceRepository::modified_service(modify_service_repository_token);

        let config = JwtConfig::default_testing();

        let r = sign_jwt(config, user_id).unwrap();

        let rs = service.refresh_user_token(&r.pair.refresh_token).await;

        assert!(rs.is_ok());
    }
}
