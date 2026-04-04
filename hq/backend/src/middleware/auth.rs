use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use hq_core::{Claims, Service};
use jsonwebtoken::{DecodingKey, Validation, decode};
use std::sync::Arc;
use uuid::Uuid;

pub struct AuthUser(pub Uuid);

#[async_trait]
impl FromRequestParts<Arc<Service>> for AuthUser {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<Service>,
    ) -> Result<Self, Self::Rejection> {
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

        let user_id = Uuid::parse_str(&token_data.claims.sub).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                "Invalid user ID in token".to_string(),
            )
        })?;

        Ok(AuthUser(user_id))
    }
}
