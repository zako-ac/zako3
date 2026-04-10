use derive_more::{FromStr, Into};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorkerId(pub u16);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AeId(pub u16);

#[derive(Clone, Debug, Eq, Hash, PartialEq, Into, FromStr, Serialize, Deserialize)]
pub struct DiscordToken(pub String);
