//! Turns timestamped Opus frames arriving out of order into a steady PCM
//! stream.
//!
//! Lifted out of the protofish3 transport client so the protofish4 path can use
//! the same buffer rather than growing a second one. It is now generic over the
//! source: anything yielding `(timestamp_ms, opus_bytes)` works, whether that
//! is a QUIC transfer, a UDP receiver, or a test.
//!
//! Two things it does that a plain reorder buffer does not: it holds frames for
//! a playout delay so late arrivals still land in order, and it asks the codec
//! to conceal a gap rather than emitting silence, so a lost packet sounds like
//! a brief smear instead of a click.
//!
//! A third: it paces its source. Playback advances one frame per frame — real
//! time — so a buffer that takes everything the channel offers hoards audio it
//! can never play and, once its ceiling is reached, throws the surplus away.
//! It therefore accepts only while it is less than `max_lead_ms` ahead of the
//! play head. Frames that do not fit stay in the channel, and every hop
//! upstream — the mpsc, the QUIC transfer, the tap — is held to the rate the
//! listener consumes. A sender that cannot be slowed down, a UDP tap having no
//! back channel, still hits the frame cap, so a drop is counted and reported
//! rather than silently discarded.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

/// Nominal Opus frame length, in milliseconds.
///
/// 48 kHz, 960 samples — the framing everything in zako3 assumes.
pub const FRAME_MS: u64 = 20;

/// How often a buffer that keeps dropping may say so.
const DROP_WARN_INTERVAL: Duration = Duration::from_secs(5);

/// A frame on its way to the decoder.
#[derive(Debug, Clone)]
pub struct TimedFrame {
    pub ts_ms: u64,
    pub payload: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum JitterError {
    #[error(transparent)]
    Opus(#[from] opus::Error),
    #[error("no frame arrived for {0:?}")]
    Stalled(Duration),
}

/// How the buffer is configured.
#[derive(Debug, Clone)]
pub struct JitterConfig {
    pub sample_rate: u32,
    pub channels: opus::Channels,
    /// Nominal frame length. 20 ms for the Opus framing everything here uses.
    pub frame_size_ms: u64,
    /// How far behind the newest arrival playback runs.
    ///
    /// The entire budget for reordering and retransmission: a frame later than
    /// this is concealed rather than waited for, because holding the stream any
    /// longer is more audible than the loss.
    pub playout_delay_ms: u64,
    /// Give up if nothing arrives at all for this long.
    pub stall_timeout: Duration,
    /// Hard ceiling on buffered frames.
    ///
    /// The last resort, for a sender that cannot be paced — a UDP tap has
    /// nowhere to be told to slow down. A sender that *can* be paced never
    /// reaches it, because [`Self::max_lead_ms`] stops it first.
    pub max_buffered_frames: usize,
    /// How far ahead of the play head frames are accepted.
    ///
    /// This is the pacing knob. Above it the buffer stops reading its channel,
    /// and that is what slows a source down instead of decimating it.
    pub max_lead_ms: u64,
    /// How much audio to collect before playback starts.
    ///
    /// Holding the playout budget up front turns it from a per-frame allowance
    /// that is spent immediately into a buffer that is actually there when a
    /// packet arrives late.
    pub pre_roll_ms: u64,
}

impl Default for JitterConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            channels: opus::Channels::Stereo,
            frame_size_ms: 20,
            playout_delay_ms: 100,
            stall_timeout: Duration::from_secs(15),
            // ~15 s of audio at 20 ms a frame: the ceiling, not the working
            // set.
            max_buffered_frames: 750,
            // Half a second of lead absorbs jitter and a retransmission, and is
            // far short of the seconds a tap decoding from a file would pile up
            // on its own.
            max_lead_ms: 500,
            // The playout budget, held rather than spent.
            pre_roll_ms: 100,
        }
    }
}

/// Largest number of samples per channel Opus can emit for one packet.
const MAX_SAMPLES_PER_CHANNEL: usize = 5760;

pub struct OpusJitterBuffer {
    rx: mpsc::Receiver<TimedFrame>,
    decoder: opus::Decoder,
    buffer: BTreeMap<u64, Vec<u8>>,
    cfg: JitterConfig,
    /// The timestamp of the next frame to play. `None` until the pre-roll is
    /// satisfied and playback has started.
    next_play_ts: Option<u64>,
    /// When the first frame landed, so a source trickling slower than real time
    /// cannot hold playback back past the pre-roll budget.
    first_frame_at: Option<Instant>,
    is_eof: bool,
    dropped: u64,
    last_drop_warn: Option<Instant>,
}

impl OpusJitterBuffer {
    pub fn new(rx: mpsc::Receiver<TimedFrame>, cfg: JitterConfig) -> Result<Self, JitterError> {
        let decoder = opus::Decoder::new(cfg.sample_rate, cfg.channels)?;
        Ok(Self {
            rx,
            decoder,
            buffer: BTreeMap::new(),
            cfg,
            next_play_ts: None,
            first_frame_at: None,
            is_eof: false,
            dropped: 0,
            last_drop_warn: None,
        })
    }

    /// Frames discarded because the buffer was full.
    ///
    /// Nonzero means a sender outran playback and could not be slowed down:
    /// it is the shape of a pacing failure rather than a network one.
    pub fn dropped_frames(&self) -> u64 {
        self.dropped
    }

    /// The timestamp playback has reached, if it has started.
    ///
    /// Let the caller work out how far ahead of the listener a sender has run:
    /// frames still queued on the way here are every bit as buffered as the
    /// ones in this map, and only the caller can see both.
    pub fn playhead_ms(&self) -> Option<u64> {
        self.next_play_ts
    }

    /// Milliseconds of audio currently held ahead of the play head.
    ///
    /// Reported back to the sender so it can slow down — occupancy in
    /// milliseconds is something a sender can act on, unlike an opaque credit.
    pub fn buffered_ms(&self) -> u64 {
        let Some(newest) = self.buffer.keys().next_back().copied() else {
            return 0;
        };
        match self.next_play_ts {
            Some(next) => newest.saturating_sub(next),
            // Nothing has played yet, so there is no play head to measure
            // against; the buffer is still empty of anything playable.
            None => 0,
        }
    }

    /// The next PCM frame, or `None` at end of stream.
    pub async fn yield_pcm(&mut self) -> Result<Option<Vec<f32>>, JitterError> {
        loop {
            // Take only what fits inside the lead window. Everything else stays
            // in the channel, and the sender's next `send` is what waits — the
            // backpressure that keeps a file-paced tap at listening speed.
            self.fill_to_window();

            if self.next_play_ts.is_none() {
                if self.buffer.is_empty() && self.is_eof {
                    // Nothing ever arrived; there is nothing to play.
                    return Ok(None);
                }
                if self.ready_to_start() {
                    self.next_play_ts = self.buffer.keys().next().copied();
                    continue;
                }
                self.await_source().await?;
                continue;
            }

            let next_play_ts = self.next_play_ts.expect("just checked");
            let newest = self.buffer.keys().next_back().copied().unwrap_or(0);

            if let Some(frame) = self.buffer.remove(&next_play_ts) {
                self.next_play_ts = Some(next_play_ts + self.cfg.frame_size_ms);
                return Ok(Some(self.decode(Some(&frame))?));
            }

            if self.is_eof {
                if self.buffer.is_empty() {
                    return Ok(None);
                }
                // The stream has ended, so a gap will never fill. Jump to what
                // is actually there rather than concealing forward through
                // silence that is not coming.
                self.next_play_ts = self.buffer.keys().next().copied();
                continue;
            }

            // The playout budget for this frame is spent: conceal it and move
            // on, rather than stalling everything behind it.
            if newest.saturating_sub(next_play_ts) >= self.cfg.playout_delay_ms {
                self.next_play_ts = Some(next_play_ts + self.cfg.frame_size_ms);
                return Ok(Some(self.decode(None)?));
            }

            self.await_source().await?;
        }
    }

    /// Pull frames out of the channel until the lead window is full.
    ///
    /// Returning to the caller with the channel still occupied is the point:
    /// the frames are not lost, they are simply not this buffer's yet.
    fn fill_to_window(&mut self) {
        let window = self.max_lead_frames();
        while self.buffer.len() < window {
            match self.rx.try_recv() {
                Ok(frame) => self.accept(frame),
                // Empty or closed: either way there is nothing to take. A
                // closed channel is deliberately *not* recorded as end of
                // stream here — only when playback would otherwise wait for a
                // frame does the gap become unfillable. Marking it now would
                // turn a mid-stream hole into a forward jump, moving every
                // later frame earlier by the length of the hole.
                Err(_) => break,
            }
        }
    }

    /// Whether enough is held to start playing.
    fn ready_to_start(&self) -> bool {
        let Some(&first) = self.buffer.keys().next() else {
            return false;
        };
        if self.is_eof || self.cfg.pre_roll_ms == 0 {
            return true;
        }

        let newest = self.buffer.keys().next_back().copied().unwrap_or(first);
        let held = newest.saturating_sub(first) + self.cfg.frame_size_ms;
        if held >= self.cfg.pre_roll_ms {
            return true;
        }

        // A source trickling slower than real time would otherwise hold
        // playback back indefinitely, so the pre-roll is a budget as well as a
        // threshold.
        self.first_frame_at
            .map(|t| t.elapsed() >= Duration::from_millis(self.cfg.pre_roll_ms))
            .unwrap_or(false)
    }

    /// Wait for the source, recovering from a gap that is not the end.
    async fn await_source(&mut self) -> Result<(), JitterError> {
        match tokio::time::timeout(self.cfg.stall_timeout, self.rx.recv()).await {
            Ok(Some(frame)) => {
                self.accept(frame);
                Ok(())
            }
            Ok(None) => {
                self.is_eof = true;
                Ok(())
            }
            Err(_) => {
                if self.recover_from_stall() {
                    Ok(())
                } else {
                    Err(JitterError::Stalled(self.cfg.stall_timeout))
                }
            }
        }
    }

    /// Resume after the source went quiet, rather than ending the track.
    ///
    /// A gap that outlasts the stall timeout and still has audio behind it is a
    /// wedge somewhere upstream, not the end of the stream; skipping to the
    /// newest frame kept the rest of the track audible. With nothing buffered
    /// there is genuinely nothing left to play, and the caller is told so.
    fn recover_from_stall(&mut self) -> bool {
        let Some(newest) = self.buffer.keys().next_back().copied() else {
            return false;
        };
        tracing::warn!(
            stalled_ms = self.cfg.stall_timeout.as_millis() as u64,
            buffered_ms = self.buffered_ms(),
            "source went quiet; resuming at the newest buffered frame"
        );
        match self.next_play_ts {
            Some(_) => self.next_play_ts = Some(newest),
            None => self.next_play_ts = self.buffer.keys().next().copied(),
        }
        true
    }

    fn max_lead_frames(&self) -> usize {
        (self.cfg.max_lead_ms / self.cfg.frame_size_ms).max(1) as usize
    }

    fn accept(&mut self, frame: TimedFrame) {
        // Already played past. Common and harmless: a retransmission that
        // arrived after its slot was concealed.
        if let Some(next) = self.next_play_ts
            && frame.ts_ms < next
        {
            return;
        }

        if self.buffer.len() >= self.cfg.max_buffered_frames {
            // Drop the newest rather than the oldest: the oldest is what plays
            // next, and discarding it would turn a pacing problem into an
            // audible one. Only a sender that ignores the lead window gets
            // here, so say so instead of accounting for it silently.
            self.dropped += 1;
            let now = Instant::now();
            if self
                .last_drop_warn
                .is_none_or(|last| now.duration_since(last) >= DROP_WARN_INTERVAL)
            {
                self.last_drop_warn = Some(now);
                tracing::warn!(
                    dropped = self.dropped,
                    buffered_ms = self.buffered_ms(),
                    capacity = self.cfg.max_buffered_frames,
                    "jitter buffer full: dropping frames rather than stalling playback"
                );
            }
            return;
        }

        if self.first_frame_at.is_none() {
            self.first_frame_at = Some(Instant::now());
        }
        self.buffer.insert(frame.ts_ms, frame.payload);
    }

    /// Decode a frame, or ask Opus to conceal a missing one.
    fn decode(&mut self, frame: Option<&[u8]>) -> Result<Vec<f32>, JitterError> {
        let channels = self.cfg.channels as usize;
        let decoded = match frame {
            Some(bytes) => {
                // A real packet carries its own duration, so give the decoder
                // room for the largest one Opus can produce.
                let mut pcm = vec![0f32; MAX_SAMPLES_PER_CHANNEL * channels];
                let n = self.decoder.decode_float(bytes, &mut pcm, false)?;
                pcm.truncate(n * channels);
                return Ok(pcm);
            }
            // Concealment has no packet to take a duration from, so the buffer
            // size *is* the duration. Sizing it to the maximum — as the
            // protofish3 version did — makes Opus conceal 120 ms for a 20 ms
            // gap, shifting everything after it later by 100 ms per loss.
            None => self.concealment_samples(),
        };

        let mut pcm = vec![0f32; decoded * channels];
        let n = self.decoder.decode_float(&[], &mut pcm, true)?;
        pcm.truncate(n * channels);
        Ok(pcm)
    }

    /// Samples per channel in one nominal frame.
    fn concealment_samples(&self) -> usize {
        (self.cfg.sample_rate as u64 * self.cfg.frame_size_ms / 1000) as usize
    }
}
