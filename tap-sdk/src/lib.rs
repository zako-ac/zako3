pub mod builder;
pub mod error;
pub mod handler;
pub mod source;
pub mod stream;

#[cfg(feature = "auto-encode")]
pub mod encode;

pub use builder::{tap, TapBuilder};
pub use error::{SdkError, TapError};
pub use handler::TapHandler;
pub use source::AudioSource;
pub use stream::AudioStreamSender;

// Re-export message types for SDK users
pub use zakofish::types::message::{
    AttachedMetadata, AudioMetadataSuccessMessage, AudioRequestSuccessMessage,
};

// Re-export audio model types needed to build response structs
pub use zakofish::types::model::{AudioCachePolicy, AudioCacheType, AudioMetadata};
