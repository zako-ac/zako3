use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use hq_core::{Claims, Service};
use hq_types::hq::UserId;
use jsonwebtoken::{DecodingKey, Validation, decode};
use std::sync::Arc;

pub struct AuthUser(pub UserId);

#[async_trait]
impl FromRequestParts<Arc<Service>> for AuthUser {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Service>,
    ) -> Result<Self, Self::Rejection> {
        if let Some(auth_user) = parts.extensions.get::<AuthUser>() {
            return Ok(AuthUser(auth_user.0.clone()));
        }

        let auth_header = parts.headers.get("Authorization").ok_or((
            StatusCode::UNAUTHORIZED,
            "Missing Authorization header".to_string(),
        ))?;

        let token = auth_header
            .to_str()
            .map_err(|_| {
                (
                    StatusCode::UNAUTHORIZED,
                    "Invalid Authorization header".to_string(),
                )
            })?
            .strip_prefix("Bearer ")
            .ok_or((
                StatusCode::UNAUTHORIZED,
                "Invalid Authorization format".to_string(),
            ))?;

        let secret = &state.config.jwt_secret;

        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token".to_string()))?;

        let user_id = UserId(token_data.claims.sub);

        state
            .auth
            .get_user(&user_id.to_string())
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Unknown user".to_string()))?;

        Ok(AuthUser(user_id))
    }
}

pub struct OptionalAuthUser(pub Option<UserId>);

#[async_trait]
impl FromRequestParts<Arc<Service>> for OptionalAuthUser {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Service>,
    ) -> Result<Self, Self::Rejection> {
        if let Some(auth_user) = parts.extensions.get::<AuthUser>() {
            return Ok(OptionalAuthUser(Some(auth_user.0.clone())));
        }

        let auth_header = match parts.headers.get("Authorization") {
            Some(header) => header,
            None => return Ok(OptionalAuthUser(None)),
        };

        let token = match auth_header.to_str() {
            Ok(s) => match s.strip_prefix("Bearer ") {
                Some(t) => t,
                None => return Ok(OptionalAuthUser(None)),
            },
            Err(_) => return Ok(OptionalAuthUser(None)),
        };

        let secret = &state.config.jwt_secret;

        let token_data = match decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        ) {
            Ok(data) => data,
            Err(_) => return Ok(OptionalAuthUser(None)),
        };

        let user_id = UserId(token_data.claims.sub);

        if state.auth.get_user(&user_id.to_string()).await.is_err() {
            return Ok(OptionalAuthUser(None));
        }

        Ok(OptionalAuthUser(Some(user_id)))
    }
}

pub struct AdminUser(pub UserId);

#[async_trait]
impl FromRequestParts<Arc<Service>> for AdminUser {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Service>,
    ) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        // Fetch user from DB
        let user = state
            .auth
            .get_user(&auth_user.0.to_string())
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to fetch user: {}", e),
                )
            })?;

        if user.is_admin {
            Ok(AdminUser(auth_user.0))
        } else {
            Err((
                StatusCode::FORBIDDEN,
                "Admin permissions required".to_string(),
            ))
        }
    }
}
