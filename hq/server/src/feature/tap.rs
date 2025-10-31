use derive_more::{From, Into};

use crate::util::snowflake::LazySnowflake;

pub mod repository;

#[derive(Clone, Debug, PartialEq, From, Into)]
pub struct TapName(String);

#[derive(Clone, Debug, PartialEq)]
pub struct Tap {
    pub id: LazySnowflake,
    pub name: TapName,
}
