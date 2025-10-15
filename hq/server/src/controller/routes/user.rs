use axum::{Json, extract::State, response::IntoResponse};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use serde::Deserialize;

use crate::{
    controller::helper::{
        AppOkResponse, AppResponse, OkResponse, into_app_response, ok_app_response,
    },
    core::{
        app::AppState,
        auth::permission::{OwnedPermission, check_logic, check_permission},
    },
    feature::user::{User, repository::UserRepository},
    util::{error::AppResult, permission::PermissionFlags, snowflake::Snowflake},
};

#[derive(Clone, Debug, Deserialize)]
pub struct CreateUser {
    pub name: Option<String>,
    pub permissions: PermissionFlags,
}

#[utoipa::path(
    post,
    path = "/api/v1/user",
    tag = "user",
    responses(( status = 200, description = "Successful response", body = inline(OkResponse) )),
    security(
        ("bearer" = [ "admin" ])
    )
)]
pub async fn create_user(
    State(app): State<AppState>,
    Json(create_user): Json<CreateUser>,
    TypedHeader(access_token): TypedHeader<Authorization<Bearer>>,
) -> AppResponse<User> {
    check_permission(
        OwnedPermission::AdminOnly,
        access_token.token().to_string(),
        &app,
    )
    .await?;

    let user = User {
        id: Snowflake::new_now().as_lazy(),
        name: create_user.name,
        permissions: create_user.permissions,
    };

    app.db.create_user(&user).await?;

    into_app_response(user)
}

// TODO: add test
