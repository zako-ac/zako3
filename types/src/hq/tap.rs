use super::{ResourceTimestamp, UserId};
use derive_more::{Display, From, FromStr, Into};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    From,
    Into,
    PartialEq,
    Eq,
    ToSchema,
    Hash,
    FromStr,
    Display,
)]
pub struct TapId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, From, Into, PartialEq, Eq, ToSchema)]
pub struct TapName(pub String);

#[derive(
    Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema, PartialEq, Eq,
)]
#[serde(rename_all = "camelCase")]
pub enum TapOccupation {
    Official,
    Verified,
    Base,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
#[serde(tag = "type")]
pub enum TapPermission {
    #[serde(rename = "owner_only")]
    OwnerOnly,
    #[serde(rename = "public")]
    Public,
    #[serde(rename = "whitelisted")]
    Whitelisted {
        #[serde(rename = "userIds")]
        user_ids: Vec<String>,
    },
    #[serde(rename = "blacklisted")]
    Blacklisted {
        #[serde(rename = "userIds")]
        user_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub enum TapRole {
    #[serde(rename = "music")]
    Music,
    #[serde(rename = "tts")]
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
    pub roles: Vec<TapRole>,
    pub base_volume: f32,

    pub timestamp: ResourceTimestamp,
}

impl Tap {
    pub fn new(id: impl Into<String>, owner_id: impl Into<String>, name: String) -> Self {
        Self {
            id: TapId(id.into()),
            name: TapName(name),
            description: None,
            owner_id: UserId(owner_id.into()),
            occupation: TapOccupation::Base,
            permission: TapPermission::OwnerOnly,
            roles: vec![],
            base_volume: 1.0,
            timestamp: ResourceTimestamp::now(),
        }
    }
}
