# `zako3-tap-sdk` — API Design

## Overview

`zakofish` is the low-level protocol library: it manages protofish2 connections, TLS, reconnect
logic, and msgpack framing. It is kept unchanged. `zako3-tap-sdk` wraps it and exposes a clean,
implementor-facing API. The central mechanism is a private `HandlerBridge` that translates between
the SDK's ergonomic types and zakofish's wire types.

```
ytdl-tap  →  zako3-tap-sdk  →  zakofish  →  protofish2  →  Hub
                 (new)          (unchanged)
```

---

## Crate Location and Workspace

**New file:** `zako3/tap-sdk/Cargo.toml`

```toml
[package]
name    = "zako3-tap-sdk"
version = "0.1.0"
edition = "2024"

[features]
auto-encode = ["dep:symphonia", "dep:opus"]

[dependencies]
zakofish    = { workspace = true }
zako3-types = { workspace = true }
tokio       = { workspace = true }
async-trait = { workspace = true }
bytes       = "1"
protofish2  = { workspace = true }
tracing     = { workspace = true }
thiserror   = { workspace = true }
rustls      = { version = "0.23", features = ["ring"] }
rustls-pemfile = "2"

# feature-gated
symphonia = { version = "0.5", features = [
    "mp3", "flac", "aac", "ogg", "wav", "isomp4"
], optional = true }
opus = { version = "0.3", optional = true }
```

Add `"tap-sdk"` to `members` in `zako3/Cargo.toml`:
```toml
members = [ ..., "tap-sdk" ]
```

---

## Module Layout

```
tap-sdk/src/
├── lib.rs            re-exports; public prelude
├── handler.rs        TapHandler trait
├── stream.rs         AudioStreamSender
├── error.rs          TapError, SdkError
├── source.rs         AudioSource
├── builder.rs        TapBuilder + HandlerBridge (private)
└── encode/           #[cfg(feature = "auto-encode")]
    ├── mod.rs        EncodingStreamSender, decode_and_stream
    ├── decoder.rs    SymphoniaDecoder
    └── encoder.rs    OpusEncoder
```

---

## Core Types

### `AudioSource` — `source.rs`

Replaces the opaque `AudioRequestString` newtype.

```rust
/// A request identifier passed to a tap — typically a URL.
#[derive(Debug, Clone)]
pub struct AudioSource(String);

impl AudioSource {
    pub fn url(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AudioSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// Internal conversion from zakofish wire type
impl From<zako3_types::AudioRequestString> for AudioSource {
    fn from(ars: zako3_types::AudioRequestString) -> Self {
        Self(ars.to_string())
    }
}
```

---

### `TapError` — `error.rs`

Replaces `AudioRequestFailureMessage { reason: String, try_others: bool }`.
The `try_others` flag is encoded in the variant — implementors never touch it.

```rust
#[derive(Debug, thiserror::Error)]
pub enum TapError {
    /// Transient failure. The Hub will try another tap for this request.
    /// Use for: network errors, rate limits, timeouts, yt-dlp crashes.
    #[error("{0}")]
    Retriable(String),

    /// Permanent failure. The Hub will not retry on another tap.
    /// Use for: unsupported URL scheme, video unavailable, age-restricted content.
    #[error("{0}")]
    Permanent(String),
}

impl TapError {
    fn into_wire(self) -> AudioRequestFailureMessage {
        match self {
            TapError::Retriable(reason) => AudioRequestFailureMessage { reason, try_others: true },
            TapError::Permanent(reason) => AudioRequestFailureMessage { reason, try_others: false },
        }
    }
}

/// Top-level SDK error (returned from TapBuilder::run).
#[derive(Debug, thiserror::Error)]
pub enum SdkError {
    #[error("hub rejected connection: {0}")]
    Rejected(String),
    #[error("connection error: {0}")]
    Connection(#[from] zakofish::error::ZakofishError),
    #[error("tls config error: {0}")]
    Tls(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
```

---

### `AudioStreamSender` — `stream.rs`

The implementor never creates an MPSC channel. The SDK allocates one internally and passes the
send half here. The receive half is driven by the SDK bridge.

```rust
/// Opaque handle for pushing encoded Opus frames to the Hub.
///
/// Dropping this sender signals end-of-stream to the Hub.
pub struct AudioStreamSender {
    tx: mpsc::Sender<(Timestamp, Bytes)>,
}

impl AudioStreamSender {
    /// Send a single Opus frame with an explicit timestamp (milliseconds).
    ///
    /// Returns `false` if the Hub has closed the connection and frames are no
    /// longer being consumed; the caller should stop sending.
    pub async fn send_frame(&self, ts: Timestamp, data: Bytes) -> bool {
        self.tx.send((ts, data)).await.is_ok()
    }

    /// Convenience wrapper: computes `Timestamp(frame_index * 20)`.
    ///
    /// Assumes standard Opus frames: 48 kHz sample rate, 960 samples per frame,
    /// giving 20 ms per frame.
    pub async fn send_opus_frame(&self, frame_index: u64, data: Bytes) -> bool {
        self.send_frame(Timestamp(frame_index * 20), data).await
    }
}
```

---

## `TapHandler` Trait — `handler.rs`

```rust
#[async_trait::async_trait]
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
    /// Opus frame, then lets `stream` drop to signal end-of-stream.
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
```

### Comparison with old trait

```rust
// BEFORE (zakofish TapHandler)
async fn handle_audio_request(
    &self,
    ars: AudioRequestString,              // opaque newtype
    headers: HashMap<String, String>,     // always ignored
) -> Result<
    (AudioRequestSuccessMessage, mpsc::Receiver<(Timestamp, Bytes)>),
    AudioRequestFailureMessage,           // { reason, try_others: bool }
>;

// AFTER (zako3-tap-sdk TapHandler)
async fn handle_audio_request(
    &self,
    source: AudioSource,                  // named, has .as_str() / Display
    stream: AudioStreamSender,            // SDK provides; just call .send_opus_frame()
) -> Result<AudioRequestSuccessMessage, TapError>;  // Retriable / Permanent
```

---

## `TapBuilder` — `builder.rs`

Single entry point: `zako3_tap_sdk::tap()`.

```rust
pub fn tap() -> TapBuilder {
    TapBuilder::default()
}

#[derive(Default)]
pub struct TapBuilder {
    cert_pem:         Option<PathBuf>,
    hub_addr:         Option<String>,    // "host:port"
    tap_id:           Option<String>,
    friendly_name:    Option<String>,
    api_token:        Option<String>,
    selection_weight: f32,               // default: 1.0
}

impl TapBuilder {
    /// Path to the hub's root certificate PEM file.
    pub fn cert_pem(mut self, path: impl AsRef<Path>) -> Self {
        self.cert_pem = Some(path.as_ref().to_path_buf());
        self
    }

    /// Hub address as "host:port". The host part is also used as the TLS server name.
    pub fn hub(mut self, addr: impl Into<String>) -> Self {
        self.hub_addr = Some(addr.into());
        self
    }

    pub fn tap_id(mut self, id: impl Into<String>) -> Self {
        self.tap_id = Some(id.into());
        self
    }

    pub fn friendly_name(mut self, name: impl Into<String>) -> Self {
        self.friendly_name = Some(name.into());
        self
    }

    pub fn api_token(mut self, token: impl Into<String>) -> Self {
        self.api_token = Some(token.into());
        self
    }

    pub fn selection_weight(mut self, weight: f32) -> Self {
        self.selection_weight = weight;
        self
    }

    /// Connect to the Hub and block until the connection is permanently lost.
    /// Reconnection with exponential backoff is handled internally by zakofish.
    pub async fn run(self, handler: Arc<dyn TapHandler>) -> Result<(), SdkError> {
        let cert_path = self.cert_pem.expect("cert_pem is required");
        let hub_addr  = self.hub_addr.as_deref().expect("hub is required");

        // Parse "host:port"; use host as TLS server_name
        let (server_name, _) = hub_addr.rsplit_once(':').expect("hub must be host:port");

        let cert_chain  = load_certs(&cert_path)?;
        let client_config = ClientConfig {
            bind_address: "127.0.0.1:0".parse().unwrap(),
            root_certificates: cert_chain,
            supported_compression_types: vec![CompressionType::None],
            keepalive_range: Duration::from_secs(1)..Duration::from_secs(10),
            protofish_config: Default::default(),
        };

        let hello = TapClientHello {
            tap_id:           TapId::from_str(self.tap_id.as_deref().expect("tap_id required")).unwrap(),
            friendly_name:    self.friendly_name.unwrap_or_default(),
            api_token:        self.api_token.unwrap_or_default(),
            selection_weight: self.selection_weight,
        };

        let bridge = Arc::new(HandlerBridge(handler));
        let zf_tap = ZakofishTap::new(client_config)?;
        zf_tap.connect_and_run(hub_addr.parse()?, server_name, hello, bridge).await?;
        Ok(())
    }
}
```

### Internal `HandlerBridge` (private)

Never exposed publicly. Converts between SDK types and zakofish wire types.

```rust
struct HandlerBridge(Arc<dyn TapHandler>);

#[async_trait::async_trait]
impl zakofish::tap::TapHandler for HandlerBridge {
    async fn handle_audio_metadata_request(
        &self,
        ars: AudioRequestString,
        _headers: HashMap<String, String>,    // intentionally discarded
    ) -> Result<AudioMetadataSuccessMessage, AudioRequestFailureMessage> {
        self.0
            .handle_audio_metadata_request(AudioSource::from(ars))
            .await
            .map_err(TapError::into_wire)
    }

    async fn handle_audio_request(
        &self,
        ars: AudioRequestString,
        _headers: HashMap<String, String>,    // intentionally discarded
    ) -> Result<
        (AudioRequestSuccessMessage, mpsc::Receiver<(Timestamp, Bytes)>),
        AudioRequestFailureMessage,
    > {
        let (tx, rx) = mpsc::channel(32);
        let sender   = AudioStreamSender { tx };
        let source   = AudioSource::from(ars);

        self.0
            .handle_audio_request(source, sender)
            .await
            .map(|success| (success, rx))
            .map_err(TapError::into_wire)
    }
}
```

---

## `auto-encode` Feature

Adds a decode → encode pipeline for taps that produce non-Opus audio. Enabled with
`features = ["auto-encode"]` in the dependent crate.

When active, two new types and one free function are added to the public API. Nothing changes for
taps that do not enable this feature.

### `SymphoniaDecoder` — `encode/decoder.rs`

```rust
/// Wraps a symphonia `FormatReader`.
/// Decodes any format symphonia supports (MP3, FLAC, AAC, OGG/Vorbis, WAV, …)
/// into interleaved f32 PCM samples.
pub struct SymphoniaDecoder {
    format:   Box<dyn FormatReader>,
    decoder:  Box<dyn Decoder>,
    track_id: u32,
    sample_rate: u32,
    channels:    u8,
}

impl SymphoniaDecoder {
    /// Probe the format and create a decoder.
    /// `reader` can be a file, a synchronous pipe bridge, or an in-memory buffer.
    pub fn from_reader<R>(reader: R) -> Result<Self, DecodeError>
    where
        R: Read + Seek + Send + Sync + 'static;

    /// Sample rate of the decoded audio (Hz).
    pub fn sample_rate(&self) -> u32;

    /// Channel count of the decoded audio.
    pub fn channels(&self) -> u8;

    /// Decode the next packet. Returns `None` at end-of-stream.
    /// Samples are interleaved: [L0, R0, L1, R1, …] for stereo.
    pub fn next_chunk(&mut self) -> Option<Result<Vec<f32>, DecodeError>>;
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("symphonia error: {0}")]
    Symphonia(#[from] symphonia::core::errors::Error),
    #[error("no suitable audio track found")]
    NoTrack,
}
```

### `OpusEncoder` — `encode/encoder.rs`

```rust
/// Encodes interleaved f32 PCM into Opus frames.
///
/// Internally resamples to 48 kHz if necessary (via linear resampler).
/// Uses 20 ms frames (960 samples per channel at 48 kHz), which is the
/// standard Opus frame size expected by `AudioStreamSender::send_opus_frame`.
pub struct OpusEncoder {
    enc:         opus::Encoder,
    channels:    u8,
    frame_size:  usize,      // samples per channel per frame
    overflow:    Vec<f32>,   // leftover samples between push() calls
    frame_index: u64,
}

impl OpusEncoder {
    /// Create an encoder for audio with the given sample rate and channel count.
    /// `sample_rate` must be 8000, 12000, 16000, 24000, or 48000.
    pub fn new(sample_rate: u32, channels: u8) -> Result<Self, EncodeError>;

    /// Feed PCM samples (interleaved f32, range −1.0..=1.0).
    /// Returns zero or more encoded Opus frames as `(frame_index, Bytes)` pairs.
    /// Frame indices increment by 1 per call, matching `send_opus_frame`'s convention.
    pub fn push(&mut self, pcm: &[f32]) -> Result<Vec<(u64, Bytes)>, EncodeError>;

    /// Flush any remaining samples (zero-padded to a full frame).
    /// Call once after the last `push`, before dropping.
    pub fn flush(&mut self) -> Result<Vec<(u64, Bytes)>, EncodeError>;
}

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("opus error: {0}")]
    Opus(#[from] opus::Error),
    #[error("decode error: {0}")]
    Decode(#[from] DecodeError),
}
```

### `EncodingStreamSender` — `encode/mod.rs`

```rust
/// Combines `SymphoniaDecoder`, `OpusEncoder`, and `AudioStreamSender`.
/// Accepts PCM f32 slices and handles encoding + frame delivery internally.
pub struct EncodingStreamSender {
    stream:  AudioStreamSender,
    encoder: OpusEncoder,
}

impl EncodingStreamSender {
    pub fn new(
        stream:      AudioStreamSender,
        sample_rate: u32,
        channels:    u8,
    ) -> Result<Self, EncodeError>;

    /// Feed interleaved f32 PCM samples; encodes and sends all complete frames.
    /// Returns `false` if the consumer has disconnected.
    pub async fn send_pcm(&mut self, samples: &[f32]) -> Result<bool, EncodeError>;

    /// Flush the encoder and send any final partial frame.
    /// Consumes self; dropping without calling `finish` may lose the last frame.
    pub async fn finish(mut self) -> Result<bool, EncodeError>;
}

/// High-level helper: decode an entire `Read + Seek` source and stream it.
///
/// # Example
/// ```rust
/// let file = std::fs::File::open("audio.mp3")?;
/// decode_and_stream(file, stream).await?;
/// ```
pub async fn decode_and_stream<R>(
    reader: R,
    stream: AudioStreamSender,
) -> Result<(), EncodeError>
where
    R: Read + Seek + Send + Sync + 'static;
```

---

## Usage Examples

### Minimal tap (no auto-encode)

```rust
// src/main.rs
use std::sync::Arc;
use zako3_tap_sdk::tap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    rustls::crypto::ring::default_provider().install_default().ok();
    tracing_subscriber::fmt::init();

    tap()
        .cert_pem("cert.pem")
        .hub("127.0.0.1:4001")
        .tap_id("299520271348404224")
        .friendly_name("YouTube Tap")
        .api_token("zk_3eb05ee465c34ddc...")
        .selection_weight(1.0)
        .run(Arc::new(YtdlTapHandler::new().await?))
        .await?;

    Ok(())
}
```

### Implementing `TapHandler` (no auto-encode)

```rust
// src/ytdl.rs
use bytes::Bytes;
use zako3_tap_sdk::{AudioSource, AudioStreamSender, TapError, TapHandler};
use zakofish::types::message::{AudioMetadataSuccessMessage, AudioRequestSuccessMessage};
use zako3_types::{AudioCachePolicy, AudioCacheType, AudioMetadata};

#[async_trait::async_trait]
impl TapHandler for YtdlTapHandler {
    async fn handle_audio_metadata_request(
        &self,
        source: AudioSource,
    ) -> Result<AudioMetadataSuccessMessage, TapError> {
        let video = self.downloader.fetch_video_infos(source.as_str())
            .await
            .map_err(|e| TapError::Retriable(e.to_string()))?;

        let mut metadatas = vec![AudioMetadata::Title(video.title.clone())];
        if let Some(ch) = &video.channel {
            metadatas.push(AudioMetadata::Artist(ch.clone()));
        }

        Ok(AudioMetadataSuccessMessage {
            metadatas,
            cache: AudioCachePolicy { cache_type: AudioCacheType::ARHash, ttl_seconds: Some(300) },
        })
    }

    async fn handle_audio_request(
        &self,
        source: AudioSource,
        stream: AudioStreamSender,
    ) -> Result<AudioRequestSuccessMessage, TapError> {
        let url = source.to_string();

        let video = self.downloader.fetch_video_infos(&url)
            .await
            .map_err(|e| TapError::Retriable(e.to_string()))?;

        let duration_secs  = video.duration.map(|d| d as f32);
        let title_metadata = vec![AudioMetadata::Title(video.title.clone())];

        tokio::spawn(async move {
            // yt-dlp → stdout → ffmpeg → ogg → send_opus_frame
            let mut frame_index = 0u64;
            // ... (existing yt-dlp/ffmpeg pipeline) ...
            while let Some(Ok(packet)) = reader.next().await {
                if packet.data.starts_with(b"OpusHead") || packet.data.starts_with(b"OpusTags") {
                    continue;
                }
                let data = Bytes::copy_from_slice(&packet.data);
                if !stream.send_opus_frame(frame_index, data).await {
                    break;   // Hub disconnected
                }
                frame_index += 1;
            }
        });

        Ok(AudioRequestSuccessMessage {
            cache: AudioCachePolicy { cache_type: AudioCacheType::ARHash, ttl_seconds: Some(300) },
            duration_secs,
            metadatas: title_metadata,   // was vec![] before — now populated
        })
    }
}
```

### Implementing `TapHandler` with `auto-encode`

```rust
// With features = ["auto-encode"]
// The ffmpeg + ogg pipeline is replaced entirely.
use zako3_tap_sdk::encode::decode_and_stream;
use tokio_util::io::SyncIoBridge;

async fn handle_audio_request(
    &self,
    source: AudioSource,
    stream: AudioStreamSender,
) -> Result<AudioRequestSuccessMessage, TapError> {
    let url = source.to_string();

    let video = self.downloader.fetch_video_infos(&url)
        .await
        .map_err(|e| TapError::Retriable(e.to_string()))?;

    tokio::spawn(async move {
        // yt-dlp stdout → synchronous bridge → symphonia → opus → stream
        let mut ytdlp = Command::new(YTDLP_BIN)
            .args(["--no-playlist", "-f", "bestaudio", "-o", "-", &url])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("yt-dlp spawn failed");

        let stdout = ytdlp.stdout.take().unwrap();
        let sync_reader = SyncIoBridge::new(stdout);

        if let Err(e) = decode_and_stream(sync_reader, stream).await {
            tracing::error!("encode error: {e}");
        }
        let _ = ytdlp.wait().await;
    });

    Ok(AudioRequestSuccessMessage {
        cache: AudioCachePolicy { cache_type: AudioCacheType::ARHash, ttl_seconds: Some(300) },
        duration_secs: video.duration.map(|d| d as f32),
        metadatas: vec![AudioMetadata::Title(video.title)],
    })
}
```

---

## What `zakofish` Retains

`zakofish` is **not modified**. It remains the implementation detail:

| Item | Lives in |
|------|----------|
| protofish2 connection management | `zakofish` |
| TLS + reconnect logic | `zakofish` |
| msgpack framing | `zakofish` |
| `ZakofishTap` / `ZakofishHub` | `zakofish` |
| Low-level `TapHandler` trait | `zakofish` (not re-exported by SDK) |
| Wire message types | `zakofish::types::message` (used internally by SDK) |

The SDK's `TapHandler` trait is **distinct** from zakofish's. Implementors only see the SDK trait.

---

## `ytdl-tap` Dependency Change

```toml
# ytdl-tap/Cargo.toml — before
zakofish    = { version = "0.1.0", path = "../../projects/zako3/zakofish" }

# after (without symphonia pipeline)
zako3-tap-sdk = { version = "0.1.0", path = "../../projects/zako3/tap-sdk" }

# after (with symphonia pipeline — drops ogg, opus, webm-iterable deps)
zako3-tap-sdk = { version = "0.1.0", path = "../../projects/zako3/tap-sdk", features = ["auto-encode"] }
```

---

## Files to Create / Modify

| Action | Path |
|--------|------|
| Create | `zako3/tap-sdk/Cargo.toml` |
| Create | `zako3/tap-sdk/src/lib.rs` |
| Create | `zako3/tap-sdk/src/handler.rs` |
| Create | `zako3/tap-sdk/src/stream.rs` |
| Create | `zako3/tap-sdk/src/error.rs` |
| Create | `zako3/tap-sdk/src/source.rs` |
| Create | `zako3/tap-sdk/src/builder.rs` |
| Create | `zako3/tap-sdk/src/encode/mod.rs` |
| Create | `zako3/tap-sdk/src/encode/decoder.rs` |
| Create | `zako3/tap-sdk/src/encode/encoder.rs` |
| Modify | `zako3/Cargo.toml` — add `"tap-sdk"` to members |
| Modify | `ytdl-tap/Cargo.toml` — swap dep |
| Modify | `ytdl-tap/src/ytdl.rs` — implement new trait |
| Modify | `ytdl-tap/src/main.rs` — use builder |
| **No change** | `zako3/zakofish/**` |
