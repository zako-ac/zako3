use mockall::automock;

use crate::{core::config::Config, feature::auth::types::JwtConfig};

#[automock]
pub trait ConfigRepository: Send + Sync {
    fn config(&self) -> Config;
    fn jwt_config(&self) -> JwtConfig;
    fn debug_password_argon2(&self) -> Option<String>;
}

impl ConfigRepository for Config {
    fn config(&self) -> Config {
        self.clone()
    }

    fn jwt_config(&self) -> JwtConfig {
        self.jwt.clone()
    }

    fn debug_password_argon2(&self) -> Option<String> {
        self.debug_password_argon2.clone()
    }
}
