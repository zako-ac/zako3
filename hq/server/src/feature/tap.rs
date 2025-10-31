use derive_more::{From, Into};

use crate::util::snowflake::LazySnowflake;

#[derive(Clone, Debug, From, Into)]
pub struct TapName(String);

pub struct Tap {
    pub id: LazySnowflake,
    pub name: TapName,
}
