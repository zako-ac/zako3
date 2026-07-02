use bytes::Bytes;
use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::types::message::{
    AudioMetadataSuccessMessage, AudioRequestFailureMessage, AudioRequestSuccessMessage,
};
use crate::types::model::AudioRequestString;
use crate::types::{Timestamp, TransferMode};

#[async_trait::async_trait]
pub trait TapHandler: Send + Sync {
    /// Handle an incoming audio request.
    /// If successful, returns the success message, a receiver channel for
    /// `(Timestamp, Bytes)` chunks, and the `TransferMode` the tap wants to use
    /// (`Dual` for reliable+unreliable, `UnreliableOnly` to skip the reliable
    /// path and its backpressure/caching on the hub side).
    /// If failed, returns the failure message.
    async fn handle_audio_request(
        &self,
        ars: AudioRequestString,
        headers: HashMap<String, String>,
    ) -> std::result::Result<
        (
            AudioRequestSuccessMessage,
            mpsc::Receiver<(Timestamp, Bytes)>,
            TransferMode,
        ),
        AudioRequestFailureMessage,
    >;

    /// Handle an incoming audio metadata request.
    /// If successful, returns the success message with metadata.
    /// If failed, returns the failure message.
    async fn handle_audio_metadata_request(
        &self,
        ars: AudioRequestString,
        headers: HashMap<String, String>,
    ) -> std::result::Result<AudioMetadataSuccessMessage, AudioRequestFailureMessage>;
}
