use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImgHash(pub Vec<bool>);

impl From<imagehash::Hash> for ImgHash {
    fn from(val: imagehash::Hash) -> Self {
        ImgHash(val.bits)
    }
}

impl ImgHash {
    /// Pack the bit vector into bytes, MSB-first within each byte.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.0.len().div_ceil(8)];
        for (i, &bit) in self.0.iter().enumerate() {
            if bit {
                out[i / 8] |= 1 << (7 - (i % 8));
            }
        }
        out
    }

    /// Unpack `bit_len` bits from `bytes`, MSB-first within each byte.
    pub fn from_bytes(bytes: &[u8], bit_len: usize) -> Self {
        let mut bits = Vec::with_capacity(bit_len);
        for i in 0..bit_len {
            let byte = bytes.get(i / 8).copied().unwrap_or(0);
            bits.push((byte >> (7 - (i % 8))) & 1 == 1);
        }
        ImgHash(bits)
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
