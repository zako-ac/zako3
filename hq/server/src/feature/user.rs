pub mod repository;

use crate::util::{permission::PermissionFlags, snowflake::LazySnowflake};

#[derive(Clone, Debug, PartialEq)]
pub struct User {
    pub id: LazySnowflake,
    pub name: Option<String>,
    pub permissions: PermissionFlags,
}
