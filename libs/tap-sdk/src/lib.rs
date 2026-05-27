pub mod builder;
pub mod error;
pub mod handler;
pub mod source;
pub mod stream;

#[cfg(feature = "auto-encode")]
pub mod encode;

#[cfg(feature = "healthcheck")]
pub(crate) mod healthcheck;

pub use builder::{tap, TapBuilder, Transport};
pub use error::{SdkError, TapError};
pub use handler::TapHandler;
pub use protofish2::TransferMode;
pub use source::AudioSource;
pub use stream::AudioStreamSender;

// Re-export message types for SDK users
pub use zakofish::types::message::{
    AttachedMetadata, AudioMetadataSuccessMessage, AudioRequestSuccessMessage,
};

// Re-export audio model types needed to build response structs
pub use zakofish::types::model::{AudioCachePolicy, AudioCacheType, AudioMetadata};
