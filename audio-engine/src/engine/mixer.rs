use crossbeam::channel::{Receiver, Sender};
use ringbuf::traits::{Consumer, Producer};

use crate::{constant::BUFFER_SIZE, types::TrackId};

pub enum MixerCommand<C>
where
    C: Consumer<Item = f32>,
{
    AddSource(TrackId, C),
    RemoveSource(TrackId),
    SetVolume(TrackId, f32),
    HasSource(TrackId, tokio::sync::oneshot::Sender<bool>),
}

struct ManagedSource<C>
where
    C: Consumer<Item = f32>,
{
    track_id: TrackId,
    consumer: C,
    current_volume: f32,
    target_volume: f32,
}

fn mixer_thread<C>(cmd_rx: Receiver<MixerCommand<C>>, mut output: impl Producer<Item = f32>)
where
    C: Consumer<Item = f32> + Send,
{
    let mut sources: Vec<ManagedSource<C>> = Vec::new();

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

pub struct Mixer<C>
where
    C: Consumer<Item = f32> + Send,
{
    cmd_tx: Sender<MixerCommand<C>>,
}

pub fn create_mixer<C>(output: impl Producer<Item = f32> + Send + 'static) -> Mixer<C>
where
    C: Consumer<Item = f32> + Send + 'static,
{
    let (cmd_tx, cmd_rx) = crossbeam::channel::unbounded();

    std::thread::spawn(move || {
        mixer_thread(cmd_rx, output);
    });

    Mixer { cmd_tx }
}

impl<C> Mixer<C>
where
    C: Consumer<Item = f32> + Send,
{
    pub fn add_source(&self, track_id: TrackId, consumer: C) {
        let _ = self
            .cmd_tx
            .send(MixerCommand::AddSource(track_id, consumer));
    }

    pub fn remove_source(&self, track_id: TrackId) {
        let _ = self.cmd_tx.send(MixerCommand::RemoveSource(track_id));
    }

    pub fn set_volume(&self, track_id: TrackId, volume: f32) {
        let _ = self.cmd_tx.send(MixerCommand::SetVolume(track_id, volume));
    }

    pub async fn has_source(&self, track_id: TrackId) -> bool {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        let _ = self.cmd_tx.send(MixerCommand::HasSource(track_id, resp_tx));
        resp_rx.await.unwrap_or(false)
    }
}
