pub mod repository;
pub mod service;
pub mod types;

use serde::Serialize;
use utoipa::ToSchema;

use crate::util::{permission::PermissionFlags, snowflake::LazySnowflake};

#[derive(Clone, Debug, PartialEq, ToSchema, Serialize)]
pub struct User {
    pub id: LazySnowflake,
    pub name: Option<String>,
    pub permissions: PermissionFlags,
}
