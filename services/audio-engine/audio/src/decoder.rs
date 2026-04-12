use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use mockall::automock;
use ringbuf::traits::{Observer, Producer};
use serenity::async_trait;
use tokio::sync::watch;
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

    fn pause_track(&self, track_id: TrackId);
    fn resume_track(&self, track_id: TrackId);
    fn stop_track(&self, track_id: TrackId);
}

pub struct PcmDecoder {
    pause_txs: Arc<DashMap<TrackId, watch::Sender<bool>>>,
}

impl PcmDecoder {
    pub fn new() -> Self {
        PcmDecoder {
            pause_txs: Arc::new(DashMap::new()),
        }
    }
}

impl Default for PcmDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Decoder for PcmDecoder {
    #[instrument(skip(self, stream))]
    async fn start_decoding(
        &self,
        track_id: TrackId,
        stream: tokio::sync::mpsc::Receiver<Vec<f32>>,
    ) -> ZakoResult<RingCons> {
        let (prod, cons) = create_ringbuf_pair();

        let (pause_tx, pause_rx) = watch::channel(false);

        self.pause_txs.insert(track_id, pause_tx);

        let pause_txs = self.pause_txs.clone();

        tokio::spawn(async move {
            let result = spawn_decode_task(track_id, stream, prod, pause_rx).await;
            if let Err(e) = result {
                tracing::error!(track_id = %track_id, error = %e, "Decoding task failed");
            }

            // Clean up pause_txs entry on task exit
            pause_txs.remove(&track_id);
        });

        Ok(cons)
    }

    fn pause_track(&self, track_id: TrackId) {
        if let Some(tx) = self.pause_txs.get(&track_id) {
            let _ = tx.send(true);
        }
    }

    fn resume_track(&self, track_id: TrackId) {
        if let Some(tx) = self.pause_txs.get(&track_id) {
            let _ = tx.send(false);
        }
    }

    fn stop_track(&self, track_id: TrackId) {
        self.pause_txs.remove(&track_id);
    }
}

async fn spawn_decode_task(
    track_id: TrackId,
    mut stream: tokio::sync::mpsc::Receiver<Vec<f32>>,
    mut producer: RingProd,
    mut pause_rx: watch::Receiver<bool>,
) -> ZakoResult<()> {
    tracing::debug!(track_id = %track_id, "Starting PCM decode task");

    let speed_control_config = speed_control::SpeedControlConfig {
        min_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(100),
        target_fill_ratio: 0.5,
    };

    while let Some(chunk) = stream.recv().await {
        // Pause gate: wait until resume or stop_track
        while *pause_rx.borrow() {
            match pause_rx.changed().await {
                Ok(_) => {}
                Err(_) => {
                    // Sender dropped (stop_track called), exit cleanly
                    tracing::debug!(track_id = %track_id, "Pause sender dropped, ending decode task");
                    return Ok(());
                }
            }
        }

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
