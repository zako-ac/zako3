use axum::{
    Extension,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    response::sse::{Event, KeepAlive, Sse},
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use futures_util::StreamExt;
use hq_core::{Claims, Service};
use jsonwebtoken::{DecodingKey, Validation, decode};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

pub async fn stats_sse(
    State(service): State<Arc<Service>>,
    Extension(stats_tx): Extension<broadcast::Sender<()>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> impl IntoResponse {
    let token = auth.token();
    let secret = &service.config.jwt_secret;

    let token_data = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ) {
        Ok(data) => data,
        Err(_) => return (StatusCode::UNAUTHORIZED, "Invalid token").into_response(),
    };

    let user_id = token_data.claims.sub;
    if service.auth.get_user(&user_id).await.is_err() {
        return (StatusCode::UNAUTHORIZED, "Unknown user").into_response();
    }

    let rx = stats_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| async move {
        msg.ok()
            .map(|_| Ok::<Event, std::convert::Infallible>(Event::default().data("stats_changed")))
    });

    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}
