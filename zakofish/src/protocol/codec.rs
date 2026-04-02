use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::error::Result;

pub fn encode_msgpack<T: Serialize>(value: &T) -> Result<Bytes> {
    let mut buf = Vec::new();
    let mut serializer = rmp_serde::Serializer::new(&mut buf).with_struct_map();
    value.serialize(&mut serializer)?;
    Ok(Bytes::from(buf))
}

pub fn decode_msgpack<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T> {
    let mut de = rmp_serde::Deserializer::new(bytes);
    let value = T::deserialize(&mut de)?;
    Ok(value)
}
