use super::ResourceTimestamp;
use derive_more::{Display, From, FromStr, Into};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, Hash, From, Into, Serialize, Deserialize, ToSchema, Copy)]
pub struct UserId(pub u64);

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for UserId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    From,
    Into,
    Serialize,
    Deserialize,
    ToSchema,
    Display,
    FromStr,
)]
pub struct DiscordUserId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, From, Into, Serialize, Deserialize, ToSchema)]
pub struct Username(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct User {
    pub id: UserId,
    pub discord_user_id: DiscordUserId,
    pub username: Username,
    pub avatar_url: Option<String>,
    pub email: Option<String>,
    pub permissions: Vec<String>,
    pub timestamp: ResourceTimestamp,
}

impl User {
    pub fn new(id: u64, discord_user_id: String, username: String) -> Self {
        Self {
            id: UserId(id),
            discord_user_id: DiscordUserId(discord_user_id),
            username: Username(username),
            avatar_url: None,
            email: None,
            permissions: Vec::new(),
            timestamp: ResourceTimestamp::now(),
        }
    }
}
