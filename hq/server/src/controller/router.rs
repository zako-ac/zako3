use axum::Router;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    controller::{
        api::ApiDoc,
        routes::{auth::*, user::*},
    },
    core::app::AppState,
};

pub fn create_router(state: AppState) -> Router {
    let (router, openapi) = create_split_router();

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", openapi))
        .merge(router)
        .with_state(state)
}

pub fn create_openapi_only_router() -> Router {
    let (_, openapi) = create_split_router();

    Router::new().merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", openapi))
}

fn create_split_router() -> (Router<AppState>, utoipa::openapi::OpenApi) {
    let (router, openapi) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(
            create_user,
            get_user,
            update_user_public,
            delete_user
        ))
        .routes(routes!(update_user_permissions))
        .routes(routes!(refresh_refresh_token))
        .split_for_parts();

    (router, openapi)
}
