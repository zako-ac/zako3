use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

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

/// How long a caller waits for the mixer thread to answer a question before
/// giving up. The mixer thread answers between frames, so anything near this
/// budget means it is wedged rather than busy.
const MIXER_REPLY_TIMEOUT: Duration = Duration::from_secs(2);

pub enum MixerCommand {
    AddSource(TrackId, RingCons, TokioSender<TrackId>),
    RemoveSource(TrackId),
    SetVolume(TrackId, f32),
    HasSource(TrackId, tokio::sync::oneshot::Sender<bool>),
    HasSources(Vec<TrackId>, tokio::sync::oneshot::Sender<HashSet<TrackId>>),
    /// Which of `track_ids` have mixed no samples for at least the given idle
    /// time. Used by the session watchdog to tell a stalled track from a long
    /// one: a three-minute song is not stuck, it is three minutes long.
    IdleSources(
        Vec<TrackId>,
        Duration,
        tokio::sync::oneshot::Sender<HashSet<TrackId>>,
    ),
}

struct ManagedSource {
    track_id: TrackId,
    end_tx: TokioSender<TrackId>,
    consumer: RingCons,
    current_volume: f32,
    target_volume: f32,
    /// Last time this source handed samples to the mix. A source that stops
    /// handing over samples while it is not finished is stalled.
    last_progress: Instant,
}

/// Insert a source unless that track is already being mixed.
///
/// Returns `false` when the add was ignored, having recorded and logged it.
/// A duplicate add is not harmless: the same track would be mixed twice (the
/// audio doubles) and would push two end events, and it happens whenever
/// reconcile's `has_sources` probe times out and reports "not playing".
fn insert_source(
    sources: &mut Vec<ManagedSource>,
    track_id: TrackId,
    consumer: RingCons,
    end_tx: TokioSender<TrackId>,
) -> bool {
    if sources.iter().any(|s| s.track_id == track_id) {
        tracing::warn!(
            track_id = %track_id,
            active_sources = sources.len(),
            "Ignoring duplicate add: track is already being mixed"
        );
        metrics::record_duplicate_mixer_add();
        return false;
    }

    tracing::debug!(track_id = %track_id, "Adding source to mixer");
    sources.push(ManagedSource {
        track_id,
        consumer,
        end_tx,
        current_volume: 1.0,
        target_volume: 1.0,
        last_progress: Instant::now(),
    });
    metrics::inc_mixer_active_sources();
    true
}

struct AliveGuard(Arc<AtomicBool>);

impl Drop for AliveGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn mixer_thread(cmd_rx: Receiver<MixerCommand>, mut output: OpusProd, alive: Arc<AtomicBool>) {
    // Flips the flag the session reads whenever this thread leaves any exit
    // path, so "the mixer is dead" and "the flag is false" can never disagree.
    let _alive = AliveGuard(alive);

    let mut sources: Vec<ManagedSource> = Vec::new();
    let mut encoder = Encoder::new(SAMPLE_RATE, Channels::Stereo, Application::Audio)
        .expect("Failed to create Opus encoder");

    let frame_dur = frame_duration();
    let mut next_tick = std::time::Instant::now();

    loop {
        let loop_start = std::time::Instant::now();

        // Checked before the command drain and before the idle sleep: a mixer
        // whose output consumer is gone can never play anything again, and it
        // used to spin here forever when it had no sources to notice it with.
        if !output.read_is_held() {
            tracing::warn!("Output consumer dropped, ending mixer thread");
            break;
        }

        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                MixerCommand::AddSource(track_id, consumer, end_tx) => {
                    insert_source(&mut sources, track_id, consumer, end_tx);
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
                MixerCommand::IdleSources(track_ids, idle, resp_tx) => {
                    let now = Instant::now();
                    let idle_tracks: HashSet<TrackId> = track_ids
                        .into_iter()
                        .filter(|id| {
                            sources.iter().any(|s| {
                                s.track_id == *id && now.duration_since(s.last_progress) >= idle
                            })
                        })
                        .collect();
                    let _ = resp_tx.send(idle_tracks);
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

            if c > 0 {
                source.last_progress = Instant::now();
            }

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

        // Re-checked mid-loop: the consumer can go away while frames are being
        // mixed, and the top-of-loop check is a whole iteration away.
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
    /// The subset of `track_ids` that has mixed no samples for at least `idle`.
    ///
    /// A track with no end event is only a fault if it is also not playing: a
    /// long music track legitimately runs far past any watchdog window without
    /// ending, and stopping it would cut the song.
    async fn idle_sources(&self, track_ids: Vec<TrackId>, idle: Duration) -> HashSet<TrackId>;
}

pub struct ThreadMixer {
    cmd_tx: Sender<MixerCommand>,
    /// `false` once the mixer thread has exited (its output consumer was
    /// dropped). The session reads this to refuse to queue audio that nothing
    /// would ever pull out.
    alive: Arc<AtomicBool>,
}

impl ThreadMixer {
    /// The mixer's liveness flag: `true` until the thread exits.
    pub fn alive(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.alive)
    }
}

pub fn create_thread_mixer(output: OpusProd) -> ThreadMixer {
    let (cmd_tx, cmd_rx) = crossbeam::channel::unbounded();

    let alive = Arc::new(AtomicBool::new(true));
    let thread_alive = Arc::clone(&alive);
    std::thread::spawn(move || {
        mixer_thread(cmd_rx, output, thread_alive);
    });

    ThreadMixer { cmd_tx, alive }
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
        match tokio::time::timeout(MIXER_REPLY_TIMEOUT, resp_rx).await {
            Ok(Ok(has_source)) => has_source,
            Ok(Err(_)) => {
                tracing::warn!(
                    track_id = %track_id,
                    "Mixer dropped the has_source reply channel"
                );
                false
            }
            Err(_) => {
                tracing::warn!(
                    track_id = %track_id,
                    timeout_ms = MIXER_REPLY_TIMEOUT.as_millis(),
                    "Mixer did not answer has_source in time; assuming the track is not playing"
                );
                false
            }
        }
    }

    async fn has_sources(&self, track_ids: Vec<TrackId>) -> HashSet<TrackId> {
        let asked = track_ids.len();
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        let _ = self
            .cmd_tx
            .send(MixerCommand::HasSources(track_ids, resp_tx));
        match tokio::time::timeout(MIXER_REPLY_TIMEOUT, resp_rx).await {
            Ok(Ok(present)) => present,
            Ok(Err(_)) => {
                tracing::warn!(asked, "Mixer dropped the has_sources reply channel");
                HashSet::new()
            }
            Err(_) => {
                // Reporting "none of them are playing" makes reconcile try to
                // start every one of them again. That is safe only because the
                // mixer now ignores an add for a track it is already mixing —
                // hence the log here, which is how a timeout would otherwise go
                // unnoticed.
                tracing::warn!(
                    asked,
                    timeout_ms = MIXER_REPLY_TIMEOUT.as_millis(),
                    "Mixer did not answer has_sources in time; treating every track as not playing (duplicate adds will be ignored)"
                );
                HashSet::new()
            }
        }
    }

    async fn idle_sources(&self, track_ids: Vec<TrackId>, idle: Duration) -> HashSet<TrackId> {
        let asked = track_ids.len();
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        let _ = self
            .cmd_tx
            .send(MixerCommand::IdleSources(track_ids, idle, resp_tx));
        match tokio::time::timeout(MIXER_REPLY_TIMEOUT, resp_rx).await {
            Ok(Ok(idle_tracks)) => idle_tracks,
            Ok(Err(_)) => {
                tracing::warn!(asked, "Mixer dropped the idle_sources reply channel");
                HashSet::new()
            }
            Err(_) => {
                // No answer means we cannot prove a track is stuck, and
                // stopping a track that is in fact playing is worse than
                // leaving a stuck one for the next sweep.
                tracing::warn!(
                    asked,
                    timeout_ms = MIXER_REPLY_TIMEOUT.as_millis(),
                    "Mixer did not answer idle_sources in time; treating no track as stalled"
                );
                HashSet::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::create_ringbuf_pair;

    fn source_pair() -> (RingCons, TokioSender<TrackId>) {
        let (_prod, cons) = create_ringbuf_pair();
        let (end_tx, _end_rx) = tokio::sync::mpsc::channel(16);
        (cons, end_tx)
    }

    #[test]
    fn duplicate_add_is_ignored() {
        let mut sources: Vec<ManagedSource> = Vec::new();
        let (cons, end_tx) = source_pair();

        assert!(insert_source(&mut sources, TrackId::from(7), cons, end_tx));
        assert_eq!(sources.len(), 1);

        let (cons2, end_tx2) = source_pair();
        assert!(!insert_source(
            &mut sources,
            TrackId::from(7),
            cons2,
            end_tx2
        ));
        assert_eq!(
            sources.len(),
            1,
            "a duplicate add for the same track must not add a second source"
        );

        // A different track still gets in.
        let (cons3, end_tx3) = source_pair();
        assert!(insert_source(
            &mut sources,
            TrackId::from(8),
            cons3,
            end_tx3
        ));
        assert_eq!(sources.len(), 2);
    }

    #[test]
    fn duplicate_add_moves_the_failure_counter() {
        let before = metrics::AUDIO_METRICS
            .duplicate_mixer_adds
            .load(std::sync::atomic::Ordering::Relaxed);
        let mut sources: Vec<ManagedSource> = Vec::new();
        let (cons, end_tx) = source_pair();
        assert!(insert_source(&mut sources, TrackId::from(9), cons, end_tx));
        let (cons2, end_tx2) = source_pair();
        assert!(!insert_source(
            &mut sources,
            TrackId::from(9),
            cons2,
            end_tx2
        ));
        // `>` rather than an exact delta: the counter is process-wide, and the
        // other test in this module bumps it too when the tests run in parallel.
        assert!(
            metrics::AUDIO_METRICS
                .duplicate_mixer_adds
                .load(std::sync::atomic::Ordering::Relaxed)
                > before,
            "the duplicate-add counter must move"
        );
    }
}
