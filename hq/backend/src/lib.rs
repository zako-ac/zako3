use axum::{routing::post, Router};
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

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::auth::login_handler,
        handlers::tap::create_tap,
        handlers::tap::list_taps,
    ),
    components(
        schemas(
            hq_types::hq::CreateTapDto,
            hq_types::hq::AuthCallbackDto,
            hq_types::hq::AuthResponseDto,
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
        .route("/api/v1/auth/login", post(auth::login_handler))
        .route("/api/v1/taps", post(tap::create_tap).get(tap::list_taps))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
