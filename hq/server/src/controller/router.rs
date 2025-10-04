use crate::{controller::identity::routes::*, core::app::AppState};
use utoipa_axum::{router::OpenApiRouter, routes};

fn create_private_router() -> OpenApiRouter<AppState> {
    let router = OpenApiRouter::new().routes(routes!(create_identity));
    // TODO middleware layer

    router
}
