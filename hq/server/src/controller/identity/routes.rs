use axum::Json;
use serde::{Deserialize, Serialize};

use crate::{controller::helper::OkResponse, core::app::AppState, util::error::AppResult};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IdentityCreate {
    pub name: String,
}

#[utoipa::path(
    post,
    path = "/pr/api/v1/identity",
    tag = "identity",
    responses(( status = 200, description = "Successful response", body = inline(OkResponse) ))
)]
pub async fn create_identity(
    app: AppState,
    Json(identity_create): Json<IdentityCreate>,
) -> AppResult<Json<OkResponse>> {
    Ok(Default::default())
}
