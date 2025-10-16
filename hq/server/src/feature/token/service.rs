use async_trait::async_trait;
use mockall::automock;

use crate::{
    core::auth::{jwt::check_jwt, types::JwtPair},
    feature::service::{Service, ServiceRepository},
    util::error::AppResult,
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
        check_jwt(self.config.jwt, refresh_token);
    }
}
