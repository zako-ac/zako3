use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use crossbeam::channel::{Receiver, Sender};
use mockall::automock;
use opus::{Application, Channels, Encoder};
use ringbuf::traits::{Consumer, Observer, Producer};
use tokio::sync::mpsc::Sender as TokioSender;

use crate::{
    OpusProd, RingCons,
    constant::{BUFFER_SIZE, SAMPLE_RATE},
    frame_duration, metrics,
    types::TrackId,
};

pub enum MixerCommand {
    AddSource(TrackId, RingCons, TokioSender<TrackId>),
    RemoveSource(TrackId),
    SetVolume(TrackId, f32),
    HasSource(TrackId, tokio::sync::oneshot::Sender<bool>),
    HasSources(Vec<TrackId>, tokio::sync::oneshot::Sender<HashSet<TrackId>>),
}

struct ManagedSource {
    track_id: TrackId,
    end_tx: TokioSender<TrackId>,
    consumer: RingCons,
    current_volume: f32,
    target_volume: f32,
}

fn mixer_thread(cmd_rx: Receiver<MixerCommand>, mut output: OpusProd) {
    let mut sources: Vec<ManagedSource> = Vec::new();
    let mut encoder = Encoder::new(SAMPLE_RATE, Channels::Stereo, Application::Audio)
        .expect("Failed to create Opus encoder");

    let frame_dur = frame_duration();
    let mut next_tick = std::time::Instant::now();

    loop {
        let loop_start = std::time::Instant::now();

        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                MixerCommand::AddSource(track_id, consumer, end_tx) => {
                    tracing::debug!(track_id = %track_id, "Adding source to mixer");
                    sources.push(ManagedSource {
                        track_id,
                        consumer,
                        end_tx,
                        current_volume: 1.0,
                        target_volume: 1.0,
                    });
                    metrics::inc_mixer_active_sources();
                }
                MixerCommand::RemoveSource(track_id) => {
                    let prev_len = sources.len();
                    sources.retain(|s| s.track_id != track_id);
                    if sources.len() < prev_len {
                        tracing::debug!(track_id = %track_id, "Removed source from mixer");
                        metrics::dec_mixer_active_sources();
                    }
                }
                MixerCommand::SetVolume(track_id, volume) => {
                    if let Some(source) = sources.iter_mut().find(|s| s.track_id == track_id) {
                        source.target_volume = volume;
                    }
                }
                MixerCommand::HasSource(track_id, resp_tx) => {
                    let has_source = sources.iter().any(|s| s.track_id == track_id);
                    let _ = resp_tx.send(has_source);
                }
                MixerCommand::HasSources(track_ids, resp_tx) => {
                    let present: HashSet<TrackId> = track_ids
                        .into_iter()
                        .filter(|id| sources.iter().any(|s| s.track_id == *id))
                        .collect();
                    let _ = resp_tx.send(present);
                }
            }
        }

        if sources.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(10));
            next_tick = std::time::Instant::now();
            continue;
        }

        let mut mixed_buffer = [0f32; BUFFER_SIZE];
        let mut ended_sources: Vec<TrackId> = Vec::new();

        let mut source_buffer = [0f32; BUFFER_SIZE];

        for source in sources.iter_mut() {
            if !source.consumer.write_is_held() && source.consumer.is_empty() {
                ended_sources.push(source.track_id);
                let _ = source.end_tx.try_send(source.track_id);
                continue;
            }

            let c = source.consumer.pop_slice(&mut source_buffer);

            let start_vol = source.current_volume;
            let end_vol = source.target_volume;

            if start_vol == end_vol {
                // Fast path: Constant volume (easy to vectorize)
                for i in 0..c {
                    mixed_buffer[i] += source_buffer[i] * start_vol;
                }
            } else {
                // Ramp path: Linearly interpolate
                let diff = (end_vol - start_vol) / c as f32;
                for i in 0..c {
                    let current_v = start_vol + (diff * i as f32);
                    mixed_buffer[i] += source_buffer[i] * current_v;
                }
                source.current_volume = end_vol;
            }
        }

        for track_id in ended_sources {
            sources.retain(|s| s.track_id != track_id);
            metrics::dec_mixer_active_sources();
        }

        if !output.read_is_held() {
            tracing::warn!("Output consumer dropped, ending mixer thread");
            break;
        }

        let mut final_buffer = [0f32; BUFFER_SIZE];
        for i in 0..BUFFER_SIZE {
            final_buffer[i] = mixed_buffer[i].clamp(-1.0, 1.0);
        }

        let mut out_buf = [0u8; 4000];
        match encoder.encode_float(&final_buffer, &mut out_buf) {
            Ok(len) => {
                let _ = output.try_push(bytes::Bytes::copy_from_slice(&out_buf[..len]));
            }
            Err(e) => {
                tracing::error!("Opus encode error: {}", e);
            }
        }

        let processing_duration = loop_start.elapsed();
        metrics::record_mixer_processing_duration(processing_duration.as_secs_f64());

        let now = std::time::Instant::now();
        next_tick += frame_dur;

        if now < next_tick {
            std::thread::sleep(next_tick - now);
        } else {
            tracing::warn!(
                duration_ms = processing_duration.as_millis(),
                budget_ms = frame_dur.as_millis(),
                "Mixer loop exceeded time budget"
            );
            next_tick = std::time::Instant::now();
        }
    }
}

pub type ArcMixer = Arc<dyn Mixer>;

#[automock]
#[async_trait]
pub trait Mixer: Send + Sync + 'static {
    fn add_source(&self, track_id: TrackId, consumer: RingCons, end_tx: TokioSender<TrackId>);
    fn remove_source(&self, track_id: TrackId);
    fn set_volume(&self, track_id: TrackId, volume: f32);
    async fn has_source(&self, track_id: TrackId) -> bool;
    async fn has_sources(&self, track_ids: Vec<TrackId>) -> HashSet<TrackId>;
}

pub struct ThreadMixer {
    cmd_tx: Sender<MixerCommand>,
}

pub fn create_thread_mixer(output: OpusProd) -> ThreadMixer {
    let (cmd_tx, cmd_rx) = crossbeam::channel::unbounded();

    std::thread::spawn(move || {
        mixer_thread(cmd_rx, output);
    });

    ThreadMixer { cmd_tx }
}

#[async_trait]
impl Mixer for ThreadMixer {
    fn add_source(&self, track_id: TrackId, consumer: RingCons, end_tx: TokioSender<TrackId>) {
        let _ = self
            .cmd_tx
            .send(MixerCommand::AddSource(track_id, consumer, end_tx));
    }

    fn remove_source(&self, track_id: TrackId) {
        let _ = self.cmd_tx.send(MixerCommand::RemoveSource(track_id));
    }

    fn set_volume(&self, track_id: TrackId, volume: f32) {
        let _ = self.cmd_tx.send(MixerCommand::SetVolume(track_id, volume));
    }

    async fn has_source(&self, track_id: TrackId) -> bool {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        let _ = self.cmd_tx.send(MixerCommand::HasSource(track_id, resp_tx));
        match tokio::time::timeout(std::time::Duration::from_secs(2), resp_rx).await {
            Ok(Ok(has_source)) => has_source,
            _ => false,
        }
    }

    async fn has_sources(&self, track_ids: Vec<TrackId>) -> HashSet<TrackId> {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        let _ = self.cmd_tx.send(MixerCommand::HasSources(track_ids, resp_tx));
        match tokio::time::timeout(std::time::Duration::from_secs(2), resp_rx).await {
            Ok(Ok(present)) => present,
            _ => HashSet::new(),
        }
    }
}
