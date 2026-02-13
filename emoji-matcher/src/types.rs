use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ImgHash(Vec<bool>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmojiRegisterRequest {
    pub image_url: String,
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmojiMatchRequest {
    pub image_url: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedEmojiNotification {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum EmojiRegisterResponse {
    Success,
    AlreadyExists {
        matched_name: String,
        matched_id: String,
    },
}

#[derive(Debug)]
pub struct Emoji {
    pub hash: ImgHash,
    pub registered_id: String,
    pub name: String,
}

impl Into<ImgHash> for imagehash::Hash {
    fn into(self) -> ImgHash {
        ImgHash(self.bits)
    }
}

impl ImgHash {
    pub fn to_binary_string(&self) -> String {
        self.0
            .iter()
            .map(|&b| if b { '1' } else { '0' })
            .collect::<String>()
    }

    pub fn to_float_vec(&self) -> Vec<f32> {
        self.0.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect()
    }

    pub fn from_float_vec(vec: Vec<f32>) -> Self {
        ImgHash(vec.into_iter().map(|f| f > 0.5).collect())
    }
}
