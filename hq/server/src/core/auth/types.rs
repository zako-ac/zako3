use std::time::Duration;

use hmac::Hmac;
use sha2::Sha256;

pub type AccessToken = String;
pub type RefreshToken = String;

#[derive(Clone, Debug)]
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
