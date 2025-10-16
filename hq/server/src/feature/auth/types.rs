use std::time::Duration;

use hmac::Hmac;
use serde::Serialize;
use sha2::Sha256;
use utoipa::ToSchema;

use crate::util::snowflake::LazySnowflake;

pub type AccessToken = String;
pub type RefreshToken = String;

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct JwtPair {
    pub access_token: AccessToken,
    pub refresh_token: RefreshToken,
}

#[derive(Clone, Debug)]
pub struct JwtConfig {
    pub secret: Hmac<Sha256>,
    pub access_token_ttl: Duration,
    pub refresh_token_ttl: Duration,
}

#[derive(Clone, Debug)]
pub struct RefreshTokenMeta {
    pub user_id: LazySnowflake,
    pub refresh_token_id: LazySnowflake,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use hmac::{Hmac, Mac};

    use crate::feature::auth::types::JwtConfig;

    impl JwtConfig {
        pub fn default_testing() -> Self {
            JwtConfig {
                secret: Hmac::new_from_slice(b"muffin-is-cute").unwrap(),
                access_token_ttl: Duration::from_secs(50),
                refresh_token_ttl: Duration::from_secs(400),
            }
        }
    }
}
