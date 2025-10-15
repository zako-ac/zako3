use axum::Router;
use utoipa::openapi::{Contact, Info, OpenApi};
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::SwaggerUi;

use crate::{controller::routes::user::*, core::app::AppState};

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

fn create_split_router() -> (Router<AppState>, OpenApi) {
    let (router, mut openapi) = OpenApiRouter::new()
        .routes(routes!(
            create_user,
            get_user,
            update_user_public,
            delete_user
        ))
        .routes(routes!(update_user_permissions,))
        .split_for_parts();

    let info = Info::builder()
        .title("zako3-hq")
        .description(Some("Zako3 Headquarter API"))
        .contact(Some(
            Contact::builder()
                .name(Some("MincoMK"))
                .email(Some("mail@drchi.co.kr"))
                .build(),
        ))
        .version("v1")
        .build();

    openapi.info = info;

    (router, openapi)
}
