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

use handlers::auth;
use handlers::tap;
use handlers::users;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::auth::login_handler,
        handlers::auth::callback_handler,
        handlers::tap::create_tap,
        handlers::tap::list_taps,
        handlers::tap::get_tap,
        handlers::tap::get_tap_stats,
        handlers::users::get_me,
        handlers::users::get_my_taps,
    ),
    components(
        schemas(
            hq_types::hq::CreateTapDto,
            hq_types::hq::AuthCallbackDto,
            hq_types::hq::AuthUserDto,
            hq_types::hq::AuthResponseDto,
            hq_types::hq::LoginResponseDto,
            hq_types::hq::Tap,
            hq_types::hq::TapId,
            hq_types::hq::TapName,
            hq_types::hq::TapOccupation,
            hq_types::hq::TapPermission,
            hq_types::hq::TapRole,
            hq_types::hq::UserId,
            hq_types::hq::ResourceTimestamp,
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
    let state = Arc::new(service);

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/api/v1/auth/login", get(auth::login_handler))
        .route("/api/v1/auth/callback", get(auth::callback_handler))
        .route("/api/v1/users/me", get(users::get_me))
        .route("/api/v1/users/me/taps", get(users::get_my_taps))
        .route("/api/v1/taps", post(tap::create_tap).get(tap::list_taps))
        .route("/api/v1/taps/:id", get(tap::get_tap))
        .route("/api/v1/taps/:id/stats", get(tap::get_tap_stats))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
