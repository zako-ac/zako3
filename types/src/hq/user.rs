use super::ResourceTimestamp;
use derive_more::{Display, From, FromStr, Into};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
pub struct UserId(pub String);

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
    pub banned: bool,
    pub timestamp: ResourceTimestamp,
}

impl User {
    pub fn new(id: impl Into<String>, discord_user_id: String, username: String) -> Self {
        Self {
            id: UserId(id.into()),
            discord_user_id: DiscordUserId(discord_user_id),
            username: Username(username),
            avatar_url: None,
            email: None,
            permissions: Vec::new(),
            banned: false,
            timestamp: ResourceTimestamp::now(),
        }
    }
}
