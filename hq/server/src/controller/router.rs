use utoipa_axum::{router::OpenApiRouter, routes};

use crate::core::app::AppState;

fn create_private_router() -> OpenApiRouter<AppState> {
    let router = OpenApiRouter::new().routes(routes!(create_identity));
    // TODO middleware layer

    router
}
