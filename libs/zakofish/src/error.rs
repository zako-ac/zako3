use std::fmt::Debug;

#[derive(Debug, thiserror::Error)]
pub enum ZakofishError {
    #[error("Protofish2 connection error: {0}")]
    ProtofishConnectionError(#[from] protofish2::connection::ProtofishConnectionError),
    #[error("Protofish2 mani stream error: {0}")]
    ProtofishManiStreamError(#[from] protofish2::mani::stream::ManiStreamError),
    #[error("Protofish2 transfer send error: {0}")]
    ProtofishTransferSendError(#[from] protofish2::mani::transfer::send::TransferSendError),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] rmp_serde::encode::Error),
    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] rmp_serde::decode::Error),
    #[error("Protocol error: {0}")]
    ProtocolError(String),
}

pub type Result<T> = std::result::Result<T, ZakofishError>;
