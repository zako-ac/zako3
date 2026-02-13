use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use mockall::automock;
use ringbuf::traits::{Observer, Producer};
use serenity::async_trait;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::{audio::SampleBuffer, io::ReadOnlySource};
use tokio::io::AsyncRead;
use tokio::sync::oneshot::Sender;
use tracing::instrument;
use zako3_audio_engine_types::ZakoError;

use crate::{RingCons, RingProd, async_to_sync_read, create_ringbuf_pair, speed_control};
use crate::{error::ZakoResult, metrics, types::TrackId};

pub type ArcDecoder = Arc<dyn Decoder>;

#[automock]
#[async_trait]
pub trait Decoder: Send + Sync + 'static {
    async fn start_decoding(
        &self,
        track_id: TrackId,
        stream: Box<dyn AsyncRead + Unpin + Send + Sync>,
    ) -> ZakoResult<RingCons>;
}

pub struct SymphoniaDecoder;

#[async_trait]
impl Decoder for SymphoniaDecoder {
    #[instrument(skip(self, stream))]
    async fn start_decoding(
        &self,
        track_id: TrackId,
        stream: Box<dyn AsyncRead + Unpin + Send + Sync>,
    ) -> ZakoResult<RingCons> {
        let media_source = create_media_source(stream)?;

        let (prod, cons) = create_ringbuf_pair();
        spawn_decode_task(track_id, media_source, prod).await?;

        Ok(cons)
    }
}

fn create_media_source(
    stream: Box<dyn AsyncRead + Unpin + Send + Sync>,
) -> ZakoResult<Box<dyn symphonia::core::io::MediaSource>> {
    let reader = async_to_sync_read(stream)?;
    Ok(Box::new(ReadOnlySource::new(reader)))
}

async fn spawn_decode_task(
    track_id: TrackId,
    media_source: Box<dyn symphonia::core::io::MediaSource>,
    producer: RingProd,
) -> ZakoResult<()> {
    let (sender, receiver) = tokio::sync::oneshot::channel();

    std::thread::spawn(move || {
        let result = spawn_decode_task_raw(track_id, media_source, producer, sender);
        if let Err(e) = result {
            tracing::error!(track_id = %track_id, error = %e, "Decoding task failed to start");
        }
    });

    match tokio::time::timeout(std::time::Duration::from_secs(5), receiver).await {
        Ok(Ok(res)) => res,
        Ok(Err(_)) => {
            metrics::record_decode_error("sender_dropped");
            Err(ZakoError::Decoding("Decoding task sender dropped".into()))
        }
        Err(_) => {
            metrics::record_decode_error("startup_timeout");
            Err(ZakoError::Decoding(
                "Decoding task startup timed out".into(),
            ))
        }
    }
}

fn spawn_decode_task_raw(
    track_id: TrackId,
    media_source: Box<dyn symphonia::core::io::MediaSource>,
    mut producer: RingProd,
    sender: Sender<ZakoResult<()>>,
) -> ZakoResult<()> {
    let mss = MediaSourceStream::new(media_source, Default::default());

    let mut probed = match symphonia::default::get_probe().format(
        &Default::default(),
        mss,
        &Default::default(),
        &Default::default(),
    ) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(track_id = %track_id, error = %e, "Failed to probe media format");
            metrics::record_decode_error("format_probe");
            return Err(ZakoError::Decoding(format!("Format probe failed: {}", e)));
        }
    };

    let track = match probed.format.default_track() {
        Some(t) => t,
        None => {
            tracing::error!(track_id = %track_id, "No default track found in media source");
            metrics::record_decode_error("no_track");
            return Err(ZakoError::Decoding(
                "No default track found in media source".into(),
            ));
        }
    };

    let mut decoder =
        match symphonia::default::get_codecs().make(&track.codec_params, &Default::default()) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(track_id = %track_id, error = %e, "Failed to create codec decoder");
                metrics::record_decode_error("codec_init");
                return Err(ZakoError::Decoding(format!("Codec init failed: {}", e)));
            }
        };

    let mut sample_buf = None;

    sender
        .send(Ok(()))
        .map_err(|_| ZakoError::Decoding("Failed to send startup confirmation".into()))?;

    let mut mid_buffer = VecDeque::new();
    let mut decode_error_count = 0u32;
    const MAX_CONSECUTIVE_ERRORS: u32 = 10;

    let speed_control_config = speed_control::SpeedControlConfig {
        min_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(100),
        target_fill_ratio: 0.5,
    };

    loop {
        let packet = match probed.format.next_packet() {
            Ok(p) => {
                decode_error_count = 0;
                p
            }
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                tracing::debug!(track_id = %track_id, "End of stream reached");
                break;
            }
            Err(e) => {
                decode_error_count += 1;
                tracing::warn!(track_id = %track_id, error = %e, consecutive_errors = decode_error_count, "Failed to read packet");
                metrics::record_decode_error("io");
                if decode_error_count >= MAX_CONSECUTIVE_ERRORS {
                    tracing::error!(track_id = %track_id, "Too many consecutive packet read errors, aborting");
                    break;
                }
                continue;
            }
        };

        let decoded = match decoder.decode(&packet) {
            Ok(d) => {
                decode_error_count = 0;
                d
            }
            Err(e) => {
                decode_error_count += 1;
                tracing::warn!(track_id = %track_id, error = %e, consecutive_errors = decode_error_count, "Decode error, skipping packet");
                metrics::record_decode_error("codec");
                if decode_error_count >= MAX_CONSECUTIVE_ERRORS {
                    tracing::error!(track_id = %track_id, "Too many consecutive decode errors, aborting");
                    break;
                }
                continue;
            }
        };

        if sample_buf.is_none() {
            sample_buf = Some(SampleBuffer::<f32>::new(
                decoded.capacity() as u64,
                *decoded.spec(),
            ));
        }

        if let Some(buf) = sample_buf.as_mut() {
            buf.copy_interleaved_ref(decoded);
            let samples = buf.samples();
            mid_buffer.extend(samples);

            let buffer_len = producer.vacant_len();

            // Push as much as we can to the ring buffer
            for _ in 0..buffer_len {
                if let Some(sample) = mid_buffer.pop_front() {
                    if producer.try_push(sample).is_err() {
                        // This can only happen if the consumer was dropped, so we can just exit
                        tracing::debug!(track_id = %track_id, "Producer push failed, consumer likely dropped, ending decode task");
                        return Ok(());
                    }
                } else {
                    break;
                }
            }

            if !producer.read_is_held() {
                // consumer dropped
                tracing::debug!(track_id = %track_id, "Consumer dropped, ending decode task");
                break;
            }

            let delay = speed_control::calculate_delay(
                &speed_control_config,
                producer.occupied_len(),
                producer.capacity().into(),
            );

            std::thread::sleep(delay);
        }
    }

    // loop until the mid buffer is drained
    while !mid_buffer.is_empty() {
        let buffer_len = producer.vacant_len();

        for _ in 0..buffer_len {
            if let Some(sample) = mid_buffer.pop_front() {
                if producer.try_push(sample).is_err() {
                    tracing::debug!(track_id = %track_id, "Producer push failed during drain, consumer likely dropped, ending decode task");
                    return Ok(());
                }
            } else {
                break;
            }
        }
    }

    tracing::debug!(track_id = %track_id, "Decoding complete");
    Ok(())
}
