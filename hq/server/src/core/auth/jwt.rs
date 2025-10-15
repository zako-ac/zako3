use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use jwt::{SignWithKey, VerifyWithKey};

use crate::{
    core::auth::types::{JwtConfig, JwtPair},
    feature::token::repository::TokenRepository,
    util::{
        error::{AppError, AppResult},
        parse::parse_u64,
        snowflake::{LazySnowflake, Snowflake},
    },
};

pub async fn generate_jwt(
    token_repository: impl TokenRepository,
    config: JwtConfig,
    user_id: LazySnowflake,
) -> AppResult<JwtPair> {
    let refresh_token_id = Snowflake::new_now().as_lazy();
    let now = SystemTime::now();

    let jwt = sign_jwt_pure(config.clone(), now, refresh_token_id, user_id)?;

    token_repository
        .add_refresh_token_user(refresh_token_id, user_id, config.refresh_token_ttl)
        .await?;

    Ok(jwt)
}

pub fn check_jwt(config: JwtConfig, access_token: String) -> AppResult<Option<LazySnowflake>> {
    check_jwt_pure(config, SystemTime::now(), access_token)
}

pub async fn revoke_refresh_token(
    token_repository: impl TokenRepository,
    refresh_token_id: LazySnowflake,
) -> AppResult<()> {
    token_repository
        .delete_refresh_token_user(refresh_token_id)
        .await
}

pub(super) fn sign_jwt_pure(
    config: JwtConfig,
    now: SystemTime,
    refresh_token_id: LazySnowflake,
    user_id: LazySnowflake,
) -> AppResult<JwtPair> {
    let access_token = {
        let (iat, exp) = calculate_iat_exp(now, config.access_token_ttl)?;

        let mut claims = BTreeMap::new();
        claims.insert("sub", user_id.to_string());
        claims.insert("iat", iat.to_string());
        claims.insert("exp", exp.to_string());

        claims.sign_with_key(&config.secret)?
    };

    let refresh_token = {
        let (iat, exp) = calculate_iat_exp(now, config.refresh_token_ttl)?;

        let mut claims = BTreeMap::new();
        claims.insert("jti", refresh_token_id.to_string());
        claims.insert("sub", user_id.to_string());
        claims.insert("iat", iat.to_string());
        claims.insert("exp", exp.to_string());

        claims.sign_with_key(&config.secret)?
    };

    Ok(JwtPair {
        access_token,
        refresh_token,
    })
}

fn calculate_iat_exp(now: SystemTime, ttl: Duration) -> AppResult<(u64, u64)> {
    let future = now + ttl;
    let iat_secs = now.duration_since(UNIX_EPOCH)?.as_secs();
    let exp_secs = future.duration_since(UNIX_EPOCH)?.as_secs();
    Ok((iat_secs, exp_secs))
}

fn check_jwt_pure(
    config: JwtConfig,
    now: SystemTime,
    access_token: String,
) -> AppResult<Option<LazySnowflake>> {
    let claims: BTreeMap<String, String> = access_token.verify_with_key(&config.secret)?;

    let user_id_str = claims
        .get("sub")
        .ok_or(AppError::Unknown(format!("expected `sub`")))?;
    let user_id = parse_u64(&user_id_str)?;

    let exp_str = claims
        .get("exp")
        .ok_or(AppError::Unknown(format!("expected `exp`")))?;
    let exp = parse_u64(exp_str)?;

    let now_secs = now.duration_since(UNIX_EPOCH)?.as_secs();

    if now_secs < exp {
        Ok(Some(user_id.into()))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use crate::{
        core::auth::jwt::{calculate_iat_exp, check_jwt_pure, sign_jwt_pure},
        util::snowflake::{LazySnowflake, Snowflake},
    };

    use hmac::{Hmac, Mac};

    use crate::core::auth::types::JwtConfig;

    #[test]
    fn test_check_jwt_success() {
        let config = JwtConfig {
            secret: Hmac::new_from_slice(b"asdf").unwrap(),
            access_token_ttl: Duration::from_secs(1),
            refresh_token_ttl: Duration::from_secs(2),
        };

        let refresh_id = Snowflake::new_now().as_lazy();
        let user_id = Snowflake::new_now().as_lazy();

        let jwt = sign_jwt_pure(config.clone(), SystemTime::now(), refresh_id, user_id).unwrap();
        let access_token = jwt.access_token;

        let got_user_id = check_jwt_pure(config.clone(), SystemTime::now(), access_token)
            .unwrap()
            .unwrap();
        assert_eq!(user_id, got_user_id);
    }

    #[test]
    fn test_check_jwt_fail() {
        let config = JwtConfig {
            secret: Hmac::new_from_slice(b"asdf").unwrap(),
            access_token_ttl: Duration::from_secs(0),
            refresh_token_ttl: Duration::from_secs(2),
        };

        let refresh_id = Snowflake::new_now().as_lazy();
        let user_id = Snowflake::new_now().as_lazy();

        let jwt = sign_jwt_pure(config.clone(), SystemTime::now(), refresh_id, user_id).unwrap();
        let access_token = jwt.access_token;

        let got_user_id = check_jwt_pure(config.clone(), SystemTime::now(), access_token).unwrap();

        assert_eq!(got_user_id, None);
    }

    #[test]
    fn test_calculate_iat_exp() {
        let now = SystemTime::now();
        let ttl = Duration::from_secs(10);

        let (iat, exp) = calculate_iat_exp(now, ttl).unwrap();

        let now_secs = now.duration_since(UNIX_EPOCH).unwrap().as_secs();

        assert_eq!(iat, now_secs);
        assert_eq!(exp, now_secs + 10);
    }
}

#[cfg(test)]
pub fn sign_jwt_testing(config: JwtConfig, user_id: LazySnowflake) -> String {
    let pair = sign_jwt_pure(
        config,
        SystemTime::now(),
        Snowflake::new_now().as_lazy(),
        user_id,
    )
    .unwrap();
    pair.access_token
}
