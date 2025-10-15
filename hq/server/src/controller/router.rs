use axum::Router;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::SwaggerUi;

use crate::{controller::routes::user::*, core::app::AppState};

pub fn create_router(state: AppState) -> Router {
    let (router, openapi) = OpenApiRouter::new()
        .routes(routes!(create_user))
        .split_for_parts();

    

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", openapi))
        .merge(router)
        .with_state(state)
}
