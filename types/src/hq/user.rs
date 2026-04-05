use super::ResourceTimestamp;
use derive_more::{From, Into};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, Hash, From, Into, Serialize, Deserialize, ToSchema, Copy)]
pub struct UserId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, From, Into, Serialize, Deserialize, ToSchema)]
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
