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

use std::collections::BTreeMap;
use std::time::Duration;

use tokio::sync::mpsc;

/// Nominal Opus frame length, in milliseconds.
///
/// 48 kHz, 960 samples — the framing everything in zako3 assumes.
pub const FRAME_MS: u64 = 20;

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
    /// Ceiling on buffered frames.
    ///
    /// Without it a sender running far ahead of realtime — which is the normal
    /// behaviour of a tap decoding from a file — grows this map without bound.
    /// The protofish3 version had no cap, so bounding only the reliable stream
    /// elsewhere would just have moved the problem here.
    pub max_buffered_frames: usize,
}

impl Default for JitterConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            channels: opus::Channels::Stereo,
            frame_size_ms: 20,
            playout_delay_ms: 100,
            stall_timeout: Duration::from_secs(15),
            // ~15 s of audio at 20 ms a frame, matching the lead a tap is
            // allowed to run.
            max_buffered_frames: 750,
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
    next_play_ts: Option<u64>,
    is_eof: bool,
    dropped: u64,
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
            is_eof: false,
            dropped: 0,
        })
    }

    /// Frames discarded because the buffer was full.
    ///
    /// Nonzero means a sender is outrunning playback, which is worth a metric:
    /// it is the shape of a pacing failure rather than a network one.
    pub fn dropped_frames(&self) -> u64 {
        self.dropped
    }

    /// Milliseconds of audio currently held.
    ///
    /// Reported back to the sender so it can slow down — occupancy in
    /// milliseconds is something a sender can act on, unlike an opaque credit.
    pub fn buffered_ms(&self) -> u64 {
        match (self.next_play_ts, self.buffer.keys().next_back()) {
            (Some(next), Some(newest)) => newest.saturating_sub(next),
            _ => 0,
        }
    }

    /// The next PCM frame, or `None` at end of stream.
    pub async fn yield_pcm(&mut self) -> Result<Option<Vec<f32>>, JitterError> {
        loop {
            // Take everything already waiting before deciding anything. Frames
            // left sitting in the channel are still buffered audio, so leaving
            // them there would make `buffered_ms` under-report and the frame
            // cap unenforceable — the sender would be told to speed up while
            // the backlog grew somewhere else.
            self.drain_ready();

            if let Some(next_play_ts) = self.next_play_ts {
                let newest = self.buffer.keys().next_back().copied().unwrap_or(0);

                if let Some(frame) = self.buffer.remove(&next_play_ts) {
                    self.next_play_ts = Some(next_play_ts + self.cfg.frame_size_ms);
                    return Ok(Some(self.decode(Some(&frame))?));
                }

                if self.is_eof {
                    if self.buffer.is_empty() {
                        return Ok(None);
                    }
                    // The stream has ended, so a gap will never fill. Jump to
                    // what is actually there rather than concealing forward
                    // through silence that is not coming.
                    self.next_play_ts = self.buffer.keys().next().copied();
                    continue;
                }

                // The playout budget for this frame is spent: conceal it and
                // move on, rather than stalling everything behind it.
                if newest.saturating_sub(next_play_ts) >= self.cfg.playout_delay_ms {
                    self.next_play_ts = Some(next_play_ts + self.cfg.frame_size_ms);
                    return Ok(Some(self.decode(None)?));
                }
            }

            match tokio::time::timeout(self.cfg.stall_timeout, self.rx.recv()).await {
                Err(_) => return Err(JitterError::Stalled(self.cfg.stall_timeout)),
                Ok(Some(frame)) => self.accept(frame),
                Ok(None) => {
                    self.is_eof = true;
                    if self.next_play_ts.is_none() {
                        // Nothing ever arrived; there is nothing to play.
                        return Ok(None);
                    }
                }
            }
        }
    }

    /// Move every immediately-available frame out of the channel.
    fn drain_ready(&mut self) {
        while let Ok(frame) = self.rx.try_recv() {
            self.accept(frame);
        }
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
            // audible one.
            self.dropped += 1;
            return;
        }

        if self.next_play_ts.is_none() {
            self.next_play_ts = Some(frame.ts_ms);
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
