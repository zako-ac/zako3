use axum::{
    Json,
    extract::{Path, State},
};
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
        auth::permission::{OwnedPermission, check_permission},
    },
    feature::user::{
        User,
        repository::{UpdateUser, UserRepository},
    },
    util::{
        error::{AppError, ResponseError},
        permission::PermissionFlags,
        snowflake::Snowflake,
    },
};

#[derive(Clone, Debug, Deserialize)]
pub struct CreateUser {
    pub name: Option<String>,
    pub permissions: PermissionFlags,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateUserName {
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateUserPermissions {
    pub permissions: PermissionFlags,
}

#[utoipa::path(
    post,
    path = "/api/v1/user",
    tag = "user",
    responses(
        ( status = 200, description = "Create user", body = User ),
        ( status = 401, description = "Unauthorized", body = ResponseError )
    ),
    security(
        ("bearer" = [ "admin" ])
    )
)]
pub async fn create_user(
    State(app): State<AppState>,
    TypedHeader(access_token): TypedHeader<Authorization<Bearer>>,
    Json(create_user): Json<CreateUser>,
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

#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}",
    tag = "user",
    params(
        ("user_id" = u64, Path, description = "ID of the user")
    ),
    responses(
        ( status = 200, description = "Get user", body = User ),
        ( status = 404, description = "User not found" )
    ),
    security(
    )
)]
pub async fn get_user(State(app): State<AppState>, Path(user_id): Path<u64>) -> AppResponse<User> {
    let user = app.db.find_user(user_id.into()).await?;

    if let Some(user) = user {
        into_app_response(user)
    } else {
        Err(AppError::NotFound)
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/user/{user_id}",
    tag = "user",
    params(
        ("user_id" = u64, Path, description = "ID of the user")
    ),
    responses(
        ( status = 200, description = "User updated", body = inline(OkResponse) ),
        ( status = 401, description = "Unauthorized", body = ResponseError ),
        ( status = 404, description = "User not found" ),
    ),
    security(
    )
)]
pub async fn update_user_public(
    State(app): State<AppState>,
    TypedHeader(access_token): TypedHeader<Authorization<Bearer>>,
    Path(user_id): Path<u64>,
    Json(update): Json<UpdateUserName>,
) -> AppOkResponse {
    check_permission(
        OwnedPermission::OwnerOnly(user_id.into()),
        access_token.token().to_string(),
        &app,
    )
    .await?;

    app.db
        .update_user(
            user_id.into(),
            &UpdateUser {
                name: Some(update.name),
                permissions: None,
            },
        )
        .await?;

    ok_app_response()
}
