use serde::Deserialize;
use utoipa::ToSchema;

use crate::util::permission::PermissionFlags;

#[derive(Clone, Debug, ToSchema, Deserialize)]
pub struct CreateUser {
    pub name: Option<String>,
    pub permissions: PermissionFlags,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct UpdateUserInfo {
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct UpdateUserPermissions {
    pub permissions: PermissionFlags,
}
