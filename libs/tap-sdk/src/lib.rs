pub mod builder;
pub mod error;
pub mod handler;
pub mod source;
pub mod stream;
pub mod watchdog;

#[cfg(feature = "auto-encode")]
pub mod encode;

#[cfg(feature = "healthcheck")]
pub(crate) mod healthcheck;

pub use builder::{TapBuilder, Transport, tap};
pub use error::{SdkError, TapError};
pub use handler::TapHandler;
pub use source::AudioSource;
pub use stream::AudioStreamSender;
pub use watchdog::{DEFAULT_REQUEST_DEADLINE, REQUEST_DEADLINE_EXIT_CODE};
pub use zakofish::{Timestamp, TransferMode};

// Re-export message types for SDK users
pub use zakofish::types::message::{
    AttachedMetadata, AudioMetadataSuccessMessage, AudioRequestSuccessMessage,
};

// Re-export audio model types needed to build response structs
pub use zakofish::types::model::{AudioCachePolicy, AudioCacheType, AudioMetadata};
