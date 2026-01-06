use crossbeam::channel::{Receiver, Sender};
use mockall::automock;
use ringbuf::traits::Consumer;

use crate::{
    constant::BUFFER_SIZE,
    types::{BoxConsumer, BoxProducer, TrackId},
};

pub enum MixerCommand {
    AddSource(TrackId, BoxConsumer),
    RemoveSource(TrackId),
    SetVolume(TrackId, f32),
    HasSource(TrackId, tokio::sync::oneshot::Sender<bool>),
}

struct ManagedSource {
    track_id: TrackId,
    consumer: Box<dyn Consumer<Item = f32> + Send>,
    current_volume: f32,
    target_volume: f32,
}

fn mixer_thread(cmd_rx: Receiver<MixerCommand>, mut output: BoxProducer) {
    let mut sources: Vec<ManagedSource> = Vec::new();

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                MixerCommand::AddSource(track_id, consumer) => {
                    sources.push(ManagedSource {
                        track_id,
                        consumer,
                        current_volume: 1.0,
                        target_volume: 1.0,
                    });
                }
                MixerCommand::RemoveSource(track_id) => {
                    sources.retain(|s| s.track_id != track_id);
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
            }
        }

        for _ in 0..BUFFER_SIZE {
            let mut mixed_sample = 0.0f32;

            sources.retain_mut(|source| {
                if let Some(sample) = source.consumer.try_pop() {
                    // Smooth volume transition
                    source.current_volume += (source.target_volume - source.current_volume) * 0.01;
                    mixed_sample += sample * source.current_volume;
                    true
                } else {
                    false
                }
            });

            let _ = output.try_push(mixed_sample);
        }
    }
}

#[automock]
pub trait Mixer {
    fn add_source(&self, track_id: TrackId, consumer: BoxConsumer);
    fn remove_source(&self, track_id: TrackId);
    fn set_volume(&self, track_id: TrackId, volume: f32);
    async fn has_source(&self, track_id: TrackId) -> bool;
}

pub struct ThreadMixer {
    cmd_tx: Sender<MixerCommand>,
}

pub fn create_thread_mixer<C>(output: BoxProducer) -> ThreadMixer {
    let (cmd_tx, cmd_rx) = crossbeam::channel::unbounded();

    std::thread::spawn(move || {
        mixer_thread(cmd_rx, output);
    });

    ThreadMixer { cmd_tx }
}

impl Mixer for ThreadMixer {
    fn add_source(&self, track_id: TrackId, consumer: BoxConsumer) {
        let _ = self
            .cmd_tx
            .send(MixerCommand::AddSource(track_id, consumer));
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
        resp_rx.await.unwrap_or(false)
    }
}
