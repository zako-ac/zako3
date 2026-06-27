//! Per-request authentication for MCP tools.
//!
//! mcpkit-axum 0.6 does not expose the HTTP `Authorization` header to tool
//! handlers (the `Context` carries no headers/extensions and tools get a
//! `NoOpPeer`). So the custom POST handler resolves the bearer token once and
//! publishes the result into a task-local that tools read via [`require_user`]
//! / [`require_admin`] / [`optional_user`]. This mirrors the REST
//! `AuthUser`/`AdminUser`/`OptionalAuthUser` extractors in
//! `crate::middleware::auth`.

use axum::http::HeaderMap;
use hq_core::{Claims, Service};
use hq_types::hq::UserId;
use jsonwebtoken::{DecodingKey, Validation, decode};
use mcpkit::types::ToolOutput;
use std::sync::Arc;

/// Resolved auth context for a single MCP request.
#[derive(Clone, Default)]
pub struct McpAuth {
    pub user: Option<UserId>,
    pub is_admin: bool,
}

tokio::task_local! {
    static MCP_AUTH: McpAuth;
}

/// Decode the `Authorization: Bearer <JWT>` header and resolve the user.
///
/// Never rejects: an absent/invalid token yields an unauthenticated context so
/// public tools still work; protected tools enforce via [`require_user`] etc.
pub(crate) async fn resolve_auth(service: &Arc<Service>, headers: &HeaderMap) -> McpAuth {
    let Some(token) = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
    else {
        return McpAuth::default();
    };

    let secret = &service.config.jwt_secret;
    let Ok(data) = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ) else {
        return McpAuth::default();
    };

    // Revocable user API keys carry a `jti`; an invalid/revoked one is treated
    // as unauthenticated so protected tools reject it.
    if let Some(jti) = &data.claims.jti
        && service.user_api_key.validate_jti(jti).await.is_err()
    {
        return McpAuth::default();
    }

    let user_id = UserId(data.claims.sub);
    match service.auth.get_user(&user_id.to_string()).await {
        Ok(user) => McpAuth {
            user: Some(user_id),
            is_admin: user.is_admin,
        },
        Err(_) => McpAuth::default(),
    }
}

/// Run a future with the given auth context bound to the task-local.
pub(crate) async fn scope<F>(auth: McpAuth, f: F) -> F::Output
where
    F: std::future::Future,
{
    MCP_AUTH.scope(auth, f).await
}

fn current_auth() -> McpAuth {
    MCP_AUTH.try_with(Clone::clone).unwrap_or_default()
}

/// Require an authenticated user, or return an auth error output.
pub(crate) fn require_user() -> Result<UserId, ToolOutput> {
    current_auth().user.ok_or_else(|| {
        ToolOutput::error("authentication required: send 'Authorization: Bearer <token>'")
    })
}

/// Require an authenticated admin, or return an auth/forbidden error output.
pub(crate) fn require_admin() -> Result<UserId, ToolOutput> {
    let auth = current_auth();
    match (auth.user, auth.is_admin) {
        (Some(uid), true) => Ok(uid),
        (Some(_), false) => Err(ToolOutput::error("admin permissions required")),
        (None, _) => Err(ToolOutput::error("authentication required")),
    }
}

/// Optional authenticated user (for endpoints with `OptionalAuthUser`).
pub(crate) fn optional_user() -> Option<UserId> {
    current_auth().user
}
