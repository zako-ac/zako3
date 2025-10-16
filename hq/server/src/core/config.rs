use std::{env, error::Error, fmt::Display, str::FromStr, time::Duration};

use hmac::{Hmac, Mac};

use crate::feature::auth::types::JwtConfig;

#[derive(Clone, Debug)]
pub struct Config {
    pub jwt: JwtConfig,
    pub postgres_connection_string: String,
    pub redis_connection_string: String,
    pub http_bind_address: String,
}

#[derive(Debug, Clone)]
pub struct LoadConfigError {
    pub field: String,
    pub message: String,
}

impl Display for LoadConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("[{}] {}", self.field, self.message))
    }
}

impl Error for LoadConfigError {}

pub fn load_config() -> Result<Config, LoadConfigError> {
    let conf = Config {
        jwt: JwtConfig {
            secret: Hmac::new_from_slice(load_env::<String>("HQ3_JWT_SECRET")?.as_bytes())
                .map_err(|e| LoadConfigError {
                    field: "HQ3_JWT_SECRET".to_string(),
                    message: e.to_string(),
                })?,
            access_token_ttl: Duration::from_secs(load_env("HQ3_JWT_ACCESS_TOKEN_TTL_SECONDS")?),
            refresh_token_ttl: Duration::from_secs(load_env("HQ3_JWT_REFRESH_TOKEN_TTL_SECONDS")?),
        },
        postgres_connection_string: load_env("HQ3_POSTGRES_CONNECTION_STRING")?,
        redis_connection_string: load_env("HQ3_REDIS_CONNECTION_STRING")?,
        http_bind_address: load_env("HQ3_HTTP_BIND_ADDR")?,
    };

    Ok(conf)
}

fn load_env<T>(env_name: &str) -> Result<T, LoadConfigError>
where
    T: FromStr,
{
    let val_str = env::var(env_name).map_err(|_| LoadConfigError {
        field: env_name.to_string(),
        message: format!("variable {} is not set.", env_name),
    })?;

    let val = val_str.parse::<T>().map_err(|_| LoadConfigError {
        field: env_name.to_string(),
        message: format!(
            "failed to parse value in variable {}: {}",
            env_name, val_str
        ),
    })?;

    Ok(val)
}
