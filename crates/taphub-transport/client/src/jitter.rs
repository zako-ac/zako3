//! Opus jitter buffer for the protofish3 client.
//!
//! protofish2 shipped an `OpusJitterBuffer` that consumed a pf2 unreliable recv
//! stream (whose chunks carried a `Timestamp`). protofish3 xfer chunks are
//! opaque `Vec<u8>` with no timestamp, so this is a port of that buffer that
//! sources chunks from a pf3 [`RecvXfer`] and recovers the timestamp from the
//! 8-byte big-endian prefix written by the server (see
//! [`zako3_taphub_transport_lib::encode_chunk`]).

use std::collections::BTreeMap;
use std::time::Duration;

use protofish3::xfer::RecvXfer;
use zako3_taphub_transport_lib::parse_chunk;

const STALL_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum JitterError {
    #[error(transparent)]
    Opus(#[from] opus::Error),
    #[error("no frame received for {}s", STALL_TIMEOUT.as_secs())]
    Stalled,
}

pub struct OpusJitterBuffer<'a> {
    receiver: RecvXfer<'a>,
    decoder: opus::Decoder,
    buffer: BTreeMap<u64, Vec<u8>>,
    frame_size_ms: u64,
    playout_delay_ms: u64,
    next_play_ts: Option<u64>,
    channels: opus::Channels,
    is_eof: bool,
}

impl<'a> OpusJitterBuffer<'a> {
    pub fn new(
        receiver: RecvXfer<'a>,
        sample_rate: u32,
        channels: opus::Channels,
        frame_size_ms: u64,
        playout_delay_ms: u64,
    ) -> Result<Self, opus::Error> {
        let decoder = opus::Decoder::new(sample_rate, channels)?;
        Ok(Self {
            receiver,
            decoder,
            buffer: BTreeMap::new(),
            frame_size_ms,
            playout_delay_ms,
            next_play_ts: None,
            channels,
            is_eof: false,
        })
    }

    /// Yields the next decoded PCM frame.
    ///
    /// Returns `Err(JitterError::Stalled)` if no frame arrives for
    /// [`STALL_TIMEOUT`].
    pub async fn yield_pcm(&mut self) -> Result<Option<Vec<f32>>, JitterError> {
        loop {
            // Buffer management
            if let Some(next_play_ts) = self.next_play_ts {
                let max_ts = self.buffer.keys().last().copied().unwrap_or(0);

                if let Some(chunk) = self.buffer.remove(&next_play_ts) {
                    let max_samples = 5760 * self.channels as usize;
                    let mut pcm = vec![0f32; max_samples];
                    let decoded_len = self.decoder.decode_float(&chunk, &mut pcm, false)?;
                    pcm.truncate(decoded_len * self.channels as usize);
                    self.next_play_ts = Some(next_play_ts + self.frame_size_ms);
                    return Ok(Some(pcm));
                } else if self.is_eof {
                    if self.buffer.is_empty() {
                        return Ok(None); // Stop if EOF and empty
                    }
                    // Skip gap: set next_play_ts to the next available ts
                    self.next_play_ts = Some(*self.buffer.keys().next().unwrap());
                    continue;
                } else if max_ts.saturating_sub(next_play_ts) >= self.playout_delay_ms {
                    let max_samples = 5760 * self.channels as usize;
                    let mut pcm = vec![0f32; max_samples];
                    let decoded_len = self.decoder.decode_float(&[], &mut pcm, true)?;
                    pcm.truncate(decoded_len * self.channels as usize);
                    self.next_play_ts = Some(next_play_ts + self.frame_size_ms);
                    return Ok(Some(pcm));
                }
            }

            // Receive next chunk with stall watchdog
            let recv_result = tokio::time::timeout(STALL_TIMEOUT, self.receiver.recv()).await;
            match recv_result {
                Err(_) => return Err(JitterError::Stalled),
                Ok(Some(data)) => {
                    let Some((ts, body)) = parse_chunk(&data) else {
                        tracing::warn!(len = data.len(), "pf3 chunk smaller than 8 bytes; dropping");
                        continue;
                    };
                    let ts = ts.0;
                    if self.next_play_ts.is_none() {
                        self.next_play_ts = Some(ts);
                    }
                    self.buffer.insert(ts, body.to_vec());
                }
                Ok(None) => {
                    self.is_eof = true;
                    // If no frames were ever received, there is nothing to play.
                    if self.next_play_ts.is_none() {
                        return Ok(None);
                    }
                }
            }
        }
    }
}
