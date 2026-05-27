use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};

use super::state::AppState;

const ADMIN_TOKEN_HEADER: &str = "x-admin-token";

/// If the server is configured with an admin token, every request must present
/// it in `x-admin-token`. If no token is configured (e.g. dev / docker-compose),
/// the middleware is a no-op.
pub async fn admin_token(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let Some(expected) = state.admin_token.as_deref() else {
        return Ok(next.run(req).await);
    };
    let presented = req
        .headers()
        .get(ADMIN_TOKEN_HEADER)
        .and_then(|v| v.to_str().ok());
    if presented == Some(expected) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
