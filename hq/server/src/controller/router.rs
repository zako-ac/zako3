use crate::controller::identity::routes::*;
use utoipa_axum::{router::OpenApiRouter, routes};

fn create_private_router() -> OpenApiRouter {
    let router = OpenApiRouter::new().routes(routes!(create_identity));
    // TODO middleware layer

    router
}
