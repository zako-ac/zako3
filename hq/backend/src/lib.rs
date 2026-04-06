use axum::{
    Router,
    routing::{get, post},
};
use hq_core::Service;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub mod handlers;
pub mod middleware;

use handlers::admin;
use handlers::api_key;
use handlers::audit_log;
use handlers::auth;
use handlers::notification;
use handlers::playback;
use handlers::tap;
use handlers::users;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::auth::login_handler,
        handlers::auth::callback_handler,
        handlers::auth::refresh_handler,
        handlers::auth::logout_handler,
        handlers::tap::create_tap,
        handlers::tap::list_taps,
        handlers::tap::get_tap,
        handlers::tap::update_tap,
        handlers::tap::admin_update_tap,
        handlers::tap::admin_update_tap_occupation,
        handlers::tap::delete_tap,
        handlers::tap::get_tap_stats,
        handlers::audit_log::get_tap_audit_logs,
        handlers::users::get_me,
        handlers::users::get_my_taps,
        handlers::users::get_my_settings,
        handlers::users::update_my_settings,
        handlers::admin::list_verification_requests,
        handlers::admin::approve_verification,
        handlers::admin::reject_verification,
        handlers::admin::list_users,
        handlers::admin::get_user,
        handlers::admin::ban_user,
        handlers::admin::unban_user,
        handlers::admin::update_user_role,
        handlers::tap::request_verification,
        handlers::notification::list_notifications,
        handlers::notification::mark_notification_read,

        handlers::playback::get_playback_state,
        handlers::playback::stop_track,
        handlers::playback::skip_music,
        handlers::playback::edit_queue,
        handlers::playback::undo_action,
        handlers::playback::get_history,
    ),
    components(
        schemas(
            hq_types::hq::CreateTapDto,
            hq_types::hq::UpdateTapDto,
            hq_types::hq::UpdateOccupationDto,
            hq_types::hq::AuthCallbackDto,
            hq_types::hq::AuthUserDto,
            hq_types::hq::AuthResponseDto,
            hq_types::hq::Tap,
            hq_types::hq::TapId,
            hq_types::hq::TapName,
            hq_types::hq::TapOccupation,
            hq_types::hq::TapPermission,
            hq_types::hq::TapRole,
            hq_types::hq::UserId,
            hq_types::hq::ResourceTimestamp,
            hq_types::hq::audit_log::AuditLogDto,
            hq_types::hq::audit_log::PaginatedAuditLogsDto,
            hq_types::hq::dtos::PaginationMetaDto,
            hq_types::hq::NotificationDto,

            hq_types::hq::CreateNotificationDto,
            handlers::admin::VerificationRequestsQuery,
            handlers::admin::PaginatedVerificationRequestsDto,
            handlers::admin::AdminUsersQuery,
            hq_types::hq::VerificationRequest,
            hq_types::hq::VerificationStatus,
            hq_types::hq::CreateVerificationRequestDto,
            hq_types::hq::RejectVerificationDto,
            hq_types::hq::UpdateUserRoleDto,
            hq_types::hq::PaginatedResponseDto<hq_types::hq::AuthUserDto>,
            hq_types::hq::settings::UserSettings,
            hq_types::hq::settings::TextMappingRule,
            hq_types::hq::settings::EmojiMappingRule,
            hq_types::hq::settings::TextReadingRule,
            hq_types::hq::settings::UserJoinLeaveAlert,

            hq_types::hq::playback::AudioMetadataDto,
            hq_types::hq::playback::TrackDto,
            hq_types::hq::playback::GuildPlaybackStateDto,
            hq_types::hq::playback::PlaybackActionDto,
            hq_types::hq::playback::StopTrackDto,
            hq_types::hq::playback::SkipDto,
            hq_types::hq::playback::QueueOperation,
            hq_types::hq::playback::EditQueueDto,
        )
    ),
    tags(
        (name = "hq", description = "HQ API")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub struct ApiDoc;

pub fn app(service: Service) -> Router {
    let state = Arc::new(service.clone());

    let rpc_impl = rpc::HqRpcImpl::new(
        service.api_key.clone(),
        service.tap.clone(),
        service.auth.clone(),
    );
    use hq_types::hq::rpc::HqRpcServer;
    let methods = rpc_impl.into_rpc();

    let admin_token = service.config.rpc_admin_token.clone();

    tokio::spawn(async move {
        let middleware =
            tower::ServiceBuilder::new().layer(rpc::AuthLayer::new(admin_token.clone()));

        let server = jsonrpsee::server::ServerBuilder::default()
            .set_http_middleware(middleware)
            .build("127.0.0.1:3001")
            .await
            .unwrap();
        let handle = server.start(methods);
        handle.stopped().await;
    });

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/api/v1/auth/login", get(auth::login_handler))
        .route("/api/v1/auth/callback", get(auth::callback_handler))
        .route("/api/v1/auth/refresh", get(auth::refresh_handler))
        .route("/api/v1/auth/logout", post(auth::logout_handler))
        .route("/api/v1/users/me", get(users::get_me))
        .route("/api/v1/users/me/taps", get(users::get_my_taps))
        .route(
            "/api/v1/users/me/settings",
            get(users::get_my_settings).put(users::update_my_settings),
        )
        .route(
            "/api/v1/admin/verifications",
            get(admin::list_verification_requests),
        )
        .route(
            "/api/v1/admin/verifications/:id/approve",
            post(admin::approve_verification),
        )
        .route(
            "/api/v1/admin/verifications/:id/reject",
            post(admin::reject_verification),
        )
        .route("/api/v1/admin/users", get(admin::list_users))
        .route("/api/v1/admin/users/:id", get(admin::get_user))
        .route("/api/v1/admin/users/:id/ban", post(admin::ban_user))
        .route("/api/v1/admin/users/:id/unban", post(admin::unban_user))
        .route(
            "/api/v1/admin/users/:id/role",
            axum::routing::patch(admin::update_user_role),
        )
        .route(
            "/api/v1/admin/taps/:id",
            axum::routing::patch(tap::admin_update_tap),
        )
        .route(
            "/api/v1/admin/taps/:id/occupation",
            axum::routing::patch(tap::admin_update_tap_occupation),
        )
        .route(
            "/api/v1/notifications",
            get(notification::list_notifications),
        )
        .route(
            "/api/v1/notifications/:id/read",
            axum::routing::patch(notification::mark_notification_read),
        )
        .route("/api/v1/taps", post(tap::create_tap).get(tap::list_taps))
        .route(
            "/api/v1/taps/:id",
            get(tap::get_tap)
                .patch(tap::update_tap)
                .delete(tap::delete_tap),
        )
        .route("/api/v1/taps/:id/verify", post(tap::request_verification))
        .route("/api/v1/taps/:id/stats", get(tap::get_tap_stats))
        .route(
            "/api/v1/taps/:id/audit-log",
            get(audit_log::get_tap_audit_logs),
        )
        .route(
            "/api/v1/taps/:id/api-tokens",
            post(api_key::create_key).get(api_key::list_keys),
        )
        .route(
            "/api/v1/taps/:id/api-tokens/:key_id",
            axum::routing::patch(api_key::update_key).delete(api_key::delete_key),
        )
        .route(
            "/api/v1/taps/:id/api-tokens/:key_id/regenerate",
            post(api_key::regenerate_key),
        )
        .route("/api/v1/playback/state", get(playback::get_playback_state))
        .route("/api/v1/playback/stop", post(playback::stop_track))
        .route("/api/v1/playback/skip", post(playback::skip_music))
        .route("/api/v1/playback/queue", axum::routing::patch(playback::edit_queue))
        .route("/api/v1/playback/undo/:action_id", post(playback::undo_action))
        .route("/api/v1/playback/history", get(playback::get_history))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
pub mod rpc;
