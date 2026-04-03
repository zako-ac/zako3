use super::{ResourceTimestamp, UserId};
use derive_more::{From, FromStr, Into};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(
    Debug, Clone, Serialize, Deserialize, From, Into, PartialEq, Eq, ToSchema, Hash, FromStr,
)]
pub struct TapId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize, From, Into, PartialEq, Eq, ToSchema)]
pub struct TapName(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TapOccupation {
    Official,
    Verified,
    Base,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case", tag = "type", content = "user_ids")]
pub enum TapPermission {
    OwnerOnly,
    Public,
    // Whitelist(Vec<DiscordUserId>), // Temporarily commented out
    // Blacklist(Vec<DiscordUserId>),
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TapRole {
    Music,
    TTS,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Tap {
    pub id: TapId,
    pub name: TapName,
    pub description: Option<String>,
    pub owner_id: UserId,
    pub occupation: TapOccupation,
    pub permission: TapPermission,
    pub role: Option<TapRole>,

    pub timestamp: ResourceTimestamp,
}

impl Tap {
    pub fn new(id: Uuid, owner_id: Uuid, name: String) -> Self {
        Self {
            id: TapId(id),
            name: TapName(name),
            description: None,
            owner_id: UserId(owner_id),
            occupation: TapOccupation::Base,
            permission: TapPermission::OwnerOnly,
            role: None,
            timestamp: ResourceTimestamp::now(),
        }
    }
}
