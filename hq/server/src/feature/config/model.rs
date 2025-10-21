use hmac::Hmac;
use sha2::Sha256;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub debug: DebugConfig,
    pub infra: InfraConfig,
    pub listen: ListenConfig,
    pub jwt: JwtConfig,
}

#[derive(Default, Clone, Debug)]
pub struct DebugConfig {
    pub debug_password_argon2: Option<String>,
}

#[derive(Clone, Debug)]
pub struct InfraConfig {
    pub postgres_connection_string: String,
    pub redis_connection_string: String,
}

#[derive(Clone, Debug)]
pub struct ListenConfig {
    pub http_bind_address: String,
}

#[derive(Clone, Debug)]
pub struct JwtConfig {
    pub secret: Hmac<Sha256>,
    pub access_token_ttl: Duration,
    pub refresh_token_ttl: Duration,
}
