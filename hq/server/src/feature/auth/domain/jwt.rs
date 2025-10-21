use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use jwt::{SignWithKey, VerifyWithKey};

use crate::{
    feature::{
        auth::domain::{
            error::AuthError,
            model::{JwtPair, RefreshTokenMeta},
        },
        config::model::JwtConfig,
    },
    util::{
        error::{AppError, AppResult},
        parse::parse_u64,
        snowflake::{LazySnowflake, Snowflake},
    },
};

pub struct SignJwtResult {
    pub pair: JwtPair,
    pub refresh_token_id: LazySnowflake,
}

/// Check JWT
///
/// # Returns
///
/// User ID
pub fn check_access_token(config: JwtConfig, access_token: String) -> AppResult<LazySnowflake> {
    Ok(check_jwt_pure(config, SystemTime::now(), access_token)?.user_id)
}

/// Check refresh token.
///
/// ## Note
///
/// This does NOT check DB!
pub fn check_refresh_token(
    config: JwtConfig,
    refresh_token: String,
) -> AppResult<RefreshTokenMeta> {
    let r = check_jwt_pure(config, SystemTime::now(), refresh_token)?;

    let refresh_id = r
        .refresh_token_id
        .ok_or(AppError::Auth(AuthError::InvalidRefreshToken))?;

    Ok(RefreshTokenMeta {
        user_id: r.user_id,
        refresh_token_id: refresh_id,
    })
}

/// Signs JWT.
///
/// # Note
/// This function does not store refresh token in DB.
pub fn sign_jwt(config: JwtConfig, user_id: LazySnowflake) -> AppResult<SignJwtResult> {
    let now = SystemTime::now();
    let refresh_token_id = Snowflake::new_now().as_lazy();

    let pair = sign_jwt_pure(config, now, refresh_token_id, user_id)?;

    Ok(SignJwtResult {
        pair,
        refresh_token_id,
    })
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

#[derive(Debug)]
struct CheckJwtResult {
    pub user_id: LazySnowflake,
    pub refresh_token_id: Option<LazySnowflake>,
}

fn check_jwt_pure(
    config: JwtConfig,
    now: SystemTime,
    access_token: String,
) -> AppResult<CheckJwtResult> {
    let claims: BTreeMap<String, String> = access_token.verify_with_key(&config.secret)?;

    let user_id_str = claims
        .get("sub")
        .ok_or(AppError::Unknown("expected `sub`".to_string()))?;
    let user_id = parse_u64(user_id_str)?;

    let exp_str = claims
        .get("exp")
        .ok_or(AppError::Unknown("expected `exp`".to_string()))?;
    let exp = parse_u64(exp_str)?;

    let now_secs = now.duration_since(UNIX_EPOCH)?.as_secs();

    if now_secs >= exp {
        return Err(AppError::Auth(AuthError::ExpiredAccessToken));
    }

    let jti_str = claims.get("jti");

    if let Some(jti_str) = jti_str {
        let refresh_token_id = parse_u64(jti_str)?;

        Ok(CheckJwtResult {
            user_id: user_id.into(),
            refresh_token_id: Some(refresh_token_id.into()),
        })
    } else {
        Ok(CheckJwtResult {
            user_id: user_id.into(),
            refresh_token_id: None,
        })
    }
}

#[cfg(test)]
pub mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use crate::{
        feature::{
            auth::domain::{
                error::AuthError,
                jwt::{calculate_iat_exp, check_jwt_pure, sign_jwt_pure},
            },
            config::model::JwtConfig,
        },
        util::{
            error::AppError,
            snowflake::{LazySnowflake, Snowflake},
        },
    };

    use hmac::{Hmac, Mac};

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

        {
            let r = check_jwt_pure(config.clone(), SystemTime::now(), jwt.access_token).unwrap();
            assert_eq!(r.user_id, user_id);
            assert_eq!(r.refresh_token_id, None);
        }

        {
            let r = check_jwt_pure(config.clone(), SystemTime::now(), jwt.refresh_token).unwrap();
            assert_eq!(r.user_id, user_id);
            assert_eq!(r.refresh_token_id, Some(refresh_id));
        }
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

        let r = check_jwt_pure(config.clone(), SystemTime::now(), access_token);

        assert!(matches!(
            r,
            Err(AppError::Auth(AuthError::ExpiredAccessToken))
        ));
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
}
