use std::ops::Deref;

use bitfield_struct::bitfield;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq)]
pub struct Snowflake {
    pub timestamp: DateTime<Utc>,
    pub worker_id: u16,
    pub trace_id: u16,
}

impl Snowflake {
    pub fn new_now() -> Self {
        Self {
            timestamp: Utc::now(),
            worker_id: 0,
            trace_id: rand::random(),
        }
    }

    pub fn serialize(&self) -> u64 {
        let bits = SnowflakeBits::new()
            .with_timestamp_seconds(self.timestamp.timestamp() as u32)
            .with_worker_id(self.worker_id)
            .with_trace_id(self.trace_id);

        bits.into_bits()
    }

    pub fn as_lazy(&self) -> LazySnowflake {
        LazySnowflake {
            value: self.serialize(),
        }
    }
}

#[bitfield(u64)]
struct SnowflakeBits {
    #[bits(32)]
    timestamp_seconds: u32,

    #[bits(16)]
    worker_id: u16,

    #[bits(16)]
    trace_id: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(transparent)]
pub struct LazySnowflake {
    value: u64,
}

impl From<u64> for LazySnowflake {
    fn from(value: u64) -> Self {
        Self { value }
    }
}

impl Deref for LazySnowflake {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl LazySnowflake {
    pub fn get(&self) -> Snowflake {
        let v = SnowflakeBits::from_bits(self.value);
        Snowflake {
            timestamp: DateTime::from_timestamp_secs(v.timestamp_seconds() as i64).unwrap(),
            worker_id: v.worker_id(),
            trace_id: v.trace_id(),
        }
    }
}

impl From<Snowflake> for u64 {
    fn from(value: Snowflake) -> Self {
        value.serialize()
    }
}

impl From<Snowflake> for LazySnowflake {
    fn from(value: Snowflake) -> Self {
        value.as_lazy()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::util::snowflake::Snowflake;

    #[test]
    fn test_snowflake_serialize() {
        let now = Utc::now();
        let snowflake = Snowflake {
            timestamp: now,
            worker_id: 42,
            trace_id: 84,
        };
        let lazy = snowflake.as_lazy();

        let flake = lazy.get();
        assert_eq!(flake.timestamp.timestamp(), snowflake.timestamp.timestamp());
        assert_eq!(flake.worker_id, snowflake.worker_id);
        assert_eq!(flake.trace_id, snowflake.trace_id);
    }
}
