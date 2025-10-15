use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{controller::routes::user::*, core::app::AppState};

fn create_router(state: AppState) -> OpenApiRouter<AppState> {
    let router = OpenApiRouter::new()
        .routes(routes!(create_user))
        .with_state(state);
    // TODO middleware layer

    router
}
