use std::{str::FromStr, time::Duration};

use hmac::{Hmac, Mac, digest::InvalidLength};
use thiserror::Error;

use crate::feature::config::model::{AppConfig, DebugConfig, InfraConfig, JwtConfig, ListenConfig};

#[derive(Clone, Error, Debug)]
pub enum LoadConfigError {
    #[error("env var error: {0}")]
    EnvVar(#[from] std::env::VarError),

    #[error("string parse error while parsing field {0} (value: {1})")]
    FromStr(String, String),

    #[error("secret has invalid length: {0}")]
    InvalidLength(#[from] InvalidLength),
}

fn load_env<T>(field: &str) -> Result<T, LoadConfigError>
where
    T: FromStr,
{
    let var = std::env::var(field)?;
    let value = var
        .parse()
        .map_err(|_| LoadConfigError::FromStr(field.to_string(), var))?;
    Ok(value)
}

fn load_env_optional<T>(field: &str) -> Option<T>
where
    T: FromStr,
{
    load_env(field).ok()
}

fn load_debug_config() -> Result<DebugConfig, LoadConfigError> {
    Ok(DebugConfig {
        debug_password_argon2: load_env_optional("HQ3_DEBUG_PASSWORD_ARGON2"),
    })
}

fn load_infra_config() -> Result<InfraConfig, LoadConfigError> {
    Ok(InfraConfig {
        postgres_connection_string: load_env("HQ3_POSTGRES_CONNECTION_STR")?,
        redis_connection_string: load_env("HQ3_REDIS_CONNECTION_STR")?,
    })
}

fn load_listen_config() -> Result<ListenConfig, LoadConfigError> {
    Ok(ListenConfig {
        http_bind_address: load_env("HQ3_HTTP_BIND_ADDRESS")?,
    })
}

fn load_jwt_config() -> Result<JwtConfig, LoadConfigError> {
    Ok(JwtConfig {
        secret: Hmac::new_from_slice(load_env::<String>("HQ3_JWT_SECRET")?.as_bytes())?,
        access_token_ttl: Duration::from_secs(load_env("HQ3_ACCESS_TOKEN_TTL_SECONDS")?),
        refresh_token_ttl: Duration::from_secs(load_env("HQ3_REFRESH_TOKEN_TTL_SECONDS")?),
    })
}

pub fn load_config() -> Result<AppConfig, LoadConfigError> {
    Ok(AppConfig {
        debug: load_debug_config()?,
        infra: load_infra_config()?,
        listen: load_listen_config()?,
        jwt: load_jwt_config()?,
    })
}
