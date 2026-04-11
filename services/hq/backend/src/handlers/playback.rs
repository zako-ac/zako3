use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, sse::{Event, KeepAlive, Sse}},
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use futures_util::StreamExt;
use hq_core::{Claims, CoreError, Service};
use hq_types::hq::playback::{
    EditQueueDto, GuildPlaybackStateDto, PauseTrackDto, PlaybackActionDto, PlaybackEvent,
    ResumeTrackDto, SkipDto, StopTrackDto,
};
use jsonwebtoken::{DecodingKey, Validation, decode};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use crate::middleware::auth::AuthUser;

fn map_error(e: CoreError) -> (StatusCode, String) {
    match e {
        CoreError::NotFound(_) => (StatusCode::NOT_FOUND, e.to_string()),
        CoreError::InvalidInput(_) => (StatusCode::BAD_REQUEST, e.to_string()),
        CoreError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, e.to_string()),
        CoreError::Forbidden(_) => (StatusCode::FORBIDDEN, e.to_string()),
        CoreError::Conflict(_) => (StatusCode::CONFLICT, e.to_string()),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn get_discord_id(
    service: &Service,
    user_id: &hq_types::hq::UserId,
) -> Result<String, (StatusCode, String)> {
    let user = service
        .auth
        .get_user(&user_id.to_string())
        .await
        .map_err(map_error)?;
    Ok(user.discord_id)
}

#[utoipa::path(
    get,
    path = "/api/v1/playback/state",
    responses(
        (status = 200, description = "Current playback state for user's guilds", body = Vec<GuildPlaybackStateDto>)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_playback_state(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<Vec<GuildPlaybackStateDto>>, (StatusCode, String)> {
    let discord_id = get_discord_id(&service, &user_id).await?;
    let states = service
        .playback
        .get_state_for_user(&discord_id)
        .await
        .map_err(map_error)?;
    Ok(Json(states))
}

#[utoipa::path(
    post,
    path = "/api/v1/playback/stop",
    request_body = StopTrackDto,
    responses(
        (status = 200, description = "Track stopped", body = PlaybackActionDto)
    ),
    security(("bearer_auth" = []))
)]
pub async fn stop_track(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Json(payload): Json<StopTrackDto>,
) -> Result<Json<PlaybackActionDto>, (StatusCode, String)> {
    let discord_id = get_discord_id(&service, &user_id).await?;
    let guild_id: u64 = payload
        .guild_id
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid guild_id".to_string()))?;
    let channel_id: u64 = payload
        .channel_id
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid channel_id".to_string()))?;
    let action = service
        .playback
        .stop_track(guild_id, channel_id, &payload.track_id, &discord_id)
        .await
        .map_err(map_error)?;
    Ok(Json(action))
}

#[utoipa::path(
    post,
    path = "/api/v1/playback/pause",
    request_body = PauseTrackDto,
    responses(
        (status = 200, description = "Track paused", body = PlaybackActionDto)
    ),
    security(("bearer_auth" = []))
)]
pub async fn pause_track(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Json(payload): Json<PauseTrackDto>,
) -> Result<Json<PlaybackActionDto>, (StatusCode, String)> {
    let discord_id = get_discord_id(&service, &user_id).await?;
    let guild_id: u64 = payload
        .guild_id
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid guild_id".to_string()))?;
    let channel_id: u64 = payload
        .channel_id
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid channel_id".to_string()))?;
    let action = service
        .playback
        .pause_track(guild_id, channel_id, &payload.track_id, &discord_id)
        .await
        .map_err(map_error)?;
    Ok(Json(action))
}

#[utoipa::path(
    post,
    path = "/api/v1/playback/resume",
    request_body = ResumeTrackDto,
    responses(
        (status = 200, description = "Track resumed", body = PlaybackActionDto)
    ),
    security(("bearer_auth" = []))
)]
pub async fn resume_track(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Json(payload): Json<ResumeTrackDto>,
) -> Result<Json<PlaybackActionDto>, (StatusCode, String)> {
    let discord_id = get_discord_id(&service, &user_id).await?;
    let guild_id: u64 = payload
        .guild_id
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid guild_id".to_string()))?;
    let channel_id: u64 = payload
        .channel_id
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid channel_id".to_string()))?;
    let action = service
        .playback
        .resume_track(guild_id, channel_id, &payload.track_id, &discord_id)
        .await
        .map_err(map_error)?;
    Ok(Json(action))
}

#[utoipa::path(
    post,
    path = "/api/v1/playback/skip",
    request_body = SkipDto,
    responses(
        (status = 200, description = "Music track skipped", body = PlaybackActionDto)
    ),
    security(("bearer_auth" = []))
)]
pub async fn skip_music(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Json(payload): Json<SkipDto>,
) -> Result<Json<PlaybackActionDto>, (StatusCode, String)> {
    let discord_id = get_discord_id(&service, &user_id).await?;
    let guild_id: u64 = payload
        .guild_id
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid guild_id".to_string()))?;
    let channel_id: u64 = payload
        .channel_id
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid channel_id".to_string()))?;
    let action = service
        .playback
        .skip_music(guild_id, channel_id, &discord_id)
        .await
        .map_err(map_error)?;
    Ok(Json(action))
}

#[utoipa::path(
    patch,
    path = "/api/v1/playback/queue",
    request_body = EditQueueDto,
    responses(
        (status = 200, description = "Queue edited", body = PlaybackActionDto)
    ),
    security(("bearer_auth" = []))
)]
pub async fn edit_queue(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Json(payload): Json<EditQueueDto>,
) -> Result<Json<PlaybackActionDto>, (StatusCode, String)> {
    let discord_id = get_discord_id(&service, &user_id).await?;
    let action = service
        .playback
        .edit_queue(payload, &discord_id)
        .await
        .map_err(map_error)?;
    Ok(Json(action))
}

#[utoipa::path(
    post,
    path = "/api/v1/playback/undo/{action_id}",
    params(
        ("action_id" = String, Path, description = "ID of the action to undo")
    ),
    responses(
        (status = 200, description = "Action undone", body = PlaybackActionDto)
    ),
    security(("bearer_auth" = []))
)]
pub async fn undo_action(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
    Path(action_id): Path<String>,
) -> Result<Json<PlaybackActionDto>, (StatusCode, String)> {
    let discord_id = get_discord_id(&service, &user_id).await?;
    let action = service
        .playback
        .undo_action(&action_id, &discord_id)
        .await
        .map_err(map_error)?;
    Ok(Json(action))
}

#[utoipa::path(
    get,
    path = "/api/v1/playback/history",
    responses(
        (status = 200, description = "Recent playback action history for user's guilds", body = Vec<PlaybackActionDto>)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_history(
    State(service): State<Arc<Service>>,
    AuthUser(user_id): AuthUser,
) -> Result<Json<Vec<PlaybackActionDto>>, (StatusCode, String)> {
    let discord_id = get_discord_id(&service, &user_id).await?;
    let history = service
        .playback
        .get_history(&discord_id, 50)
        .await
        .map_err(map_error)?;
    Ok(Json(history))
}

pub async fn playback_sse(
    State(service): State<Arc<Service>>,
    Extension(event_tx): Extension<broadcast::Sender<PlaybackEvent>>,
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

    let rx = event_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| async move {
        msg.ok().and_then(|event| {
            serde_json::to_string(&event).ok().map(|data| {
                Ok::<Event, std::convert::Infallible>(Event::default().data(data))
            })
        })
    });

    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}
