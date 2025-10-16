use async_trait::async_trait;
use mockall::automock;

use crate::{
    feature::{
        auth::{
            error::AuthError,
            jwt::{check_refresh_token, sign_jwt},
            types::JwtPair,
        },
        service::{Service, ServiceRepository},
        token::repository::TokenRepository,
    },
    util::error::{AppError, AppResult},
};

#[automock]
#[async_trait]
pub trait TokenService {
    async fn refresh_user_token(&self, access_token: &str) -> AppResult<JwtPair>;

    // TODO revoke
}

#[async_trait]
impl<S> TokenService for Service<S>
where
    S: ServiceRepository,
{
    async fn refresh_user_token(&self, refresh_token: &str) -> AppResult<JwtPair> {
        let check_result = check_refresh_token(self.config.jwt.clone(), refresh_token.to_string())?;

        let user_id = self
            .token_repo
            .get_refresh_token_user(check_result.refresh_token_id)
            .await?;

        if let Some(user_id) = user_id {
            if check_result.user_id == user_id {
                // 1
                let sign_result = sign_jwt(self.config.jwt.clone(), user_id)?;

                self.token_repo
                    .add_refresh_token_user(
                        sign_result.refresh_token_id,
                        user_id,
                        self.config.jwt.refresh_token_ttl,
                    )
                    .await?;

                // 2
                self.token_repo
                    .delete_refresh_token_user(check_result.refresh_token_id)
                    .await?;

                // 3
                Ok(sign_result.pair)
            } else {
                Err(AppError::Auth(AuthError::InsufficientPrevileges))
            }
        } else {
            Err(AppError::Auth(AuthError::InsufficientPrevileges))
        }
    }
}
