use crate::util::snowflake::LazySnowflake;

#[derive(Clone, Debug, PartialEq)]
pub struct Identity {
    pub id: LazySnowflake,
    pub name: Option<String>,
}
