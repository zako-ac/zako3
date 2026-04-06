use std::sync::Arc;
use std::time::Duration;

use mockall::automock;
use ringbuf::traits::{Observer, Producer};
use serenity::async_trait;
use tracing::instrument;

use crate::{RingCons, RingProd, create_ringbuf_pair, speed_control};
use crate::{error::ZakoResult, types::TrackId};

pub type ArcDecoder = Arc<dyn Decoder>;

#[automock]
#[async_trait]
pub trait Decoder: Send + Sync + 'static {
    async fn start_decoding(
        &self,
        track_id: TrackId,
        stream: tokio::sync::mpsc::Receiver<Vec<f32>>,
    ) -> ZakoResult<RingCons>;
}

pub struct PcmDecoder;

#[async_trait]
impl Decoder for PcmDecoder {
    #[instrument(skip(self, stream))]
    async fn start_decoding(
        &self,
        track_id: TrackId,
        stream: tokio::sync::mpsc::Receiver<Vec<f32>>,
    ) -> ZakoResult<RingCons> {
        let (prod, cons) = create_ringbuf_pair();

        tokio::spawn(async move {
            let result = spawn_decode_task(track_id, stream, prod).await;
            if let Err(e) = result {
                tracing::error!(track_id = %track_id, error = %e, "Decoding task failed");
            }
        });

        Ok(cons)
    }
}

async fn spawn_decode_task(
    track_id: TrackId,
    mut stream: tokio::sync::mpsc::Receiver<Vec<f32>>,
    mut producer: RingProd,
) -> ZakoResult<()> {
    tracing::debug!(track_id = %track_id, "Starting PCM decode task");

    let speed_control_config = speed_control::SpeedControlConfig {
        min_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(100),
        target_fill_ratio: 0.5,
    };

    while let Some(chunk) = stream.recv().await {
        let mut idx = 0;
        while idx < chunk.len() {
            if !producer.read_is_held() {
                tracing::debug!(track_id = %track_id, "Consumer dropped, ending decode task");
                return Ok(());
            }

            let vacant = producer.vacant_len();
            if vacant == 0 {
                let delay = speed_control::calculate_delay(
                    &speed_control_config,
                    producer.occupied_len(),
                    producer.capacity().into(),
                );
                tokio::time::sleep(delay).await;
                continue;
            }

            let take = vacant.min(chunk.len() - idx);

            for _ in 0..take {
                let sample = chunk[idx];
                if producer.try_push(sample).is_err() {
                    return Ok(());
                }
                idx += 1;
            }

            let delay = speed_control::calculate_delay(
                &speed_control_config,
                producer.occupied_len(),
                producer.capacity().into(),
            );
            if delay > Duration::from_millis(0) {
                tokio::time::sleep(delay).await;
            }
        }
    }

    tracing::debug!(track_id = %track_id, "Decoding complete");
    Ok(())
}
