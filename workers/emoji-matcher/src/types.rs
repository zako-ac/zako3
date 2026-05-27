use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImgHash(pub Vec<bool>);

impl From<imagehash::Hash> for ImgHash {
    fn from(val: imagehash::Hash) -> Self {
        ImgHash(val.bits)
    }
}

impl ImgHash {
    pub fn to_float_vec(&self) -> Vec<f32> {
        self.0.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect()
    }

    pub fn from_float_vec(vec: Vec<f32>) -> Self {
        ImgHash(vec.into_iter().map(|f| f > 0.5).collect())
    }

    /// Hamming distance: number of differing bits.
    pub fn hamming(&self, other: &ImgHash) -> u32 {
        if self.0.len() != other.0.len() {
            return u32::MAX;
        }
        self.0
            .iter()
            .zip(other.0.iter())
            .filter(|(a, b)| a != b)
            .count() as u32
    }
}

/// Identifier for which scope a settings row lives in.
#[derive(Debug, Clone)]
pub enum Scope {
    Global,
    Guild(String),
    User(String),
    GuildUser { user_id: String, guild_id: String },
}

impl Scope {
    /// Specificity ranking for tie-breaking when multiple scopes match.
    /// Higher number = more specific.
    pub fn specificity(&self) -> u8 {
        match self {
            Scope::Global => 0,
            Scope::Guild(_) => 1,
            Scope::User(_) => 2,
            Scope::GuildUser { .. } => 3,
        }
    }
}
