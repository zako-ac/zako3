use async_trait::async_trait;
use zakofish::types::message::{AudioMetadataSuccessMessage, AudioRequestSuccessMessage};

use crate::error::TapError;
use crate::source::AudioSource;
use crate::stream::AudioStreamSender;

#[async_trait]
pub trait TapHandler: Send + Sync {
    /// Return metadata (title, artist, …) for the given source.
    ///
    /// Called by the Hub before or independently of an audio request.
    /// The returned `AudioMetadataSuccessMessage` carries a `cache` policy
    /// that tells the Hub how long to cache this result.
    async fn handle_audio_metadata_request(
        &self,
        source: AudioSource,
    ) -> Result<AudioMetadataSuccessMessage, TapError>;

    /// Begin streaming audio for the given source.
    ///
    /// Return the success message (duration, metadata, cache policy) immediately.
    /// Spawn a task that calls `stream.send_opus_frame(index, bytes)` for each
    /// Opus frame, then let `stream` drop to signal end-of-stream.
    ///
    /// The Hub drives backpressure: `send_opus_frame` / `send_frame` will block
    /// when the internal buffer is full, and return `false` when the consumer
    /// has disconnected.
    async fn handle_audio_request(
        &self,
        source: AudioSource,
        stream: AudioStreamSender,
    ) -> Result<AudioRequestSuccessMessage, TapError>;
}
