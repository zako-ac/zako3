use std::io::{self, BufReader, Read};
use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use zako3_preload_cache::{AudioCache, FileAudioCache};
use zako3_types::cache::AudioCacheItemKey;
use zako3_types::hq::TapId;

use crate::metrics::ActionMetrics;

/// Probe all cached `.opus` files; evict entries whose files are corrupt or unreadable.
///
/// The cache uses a custom length-prefix framing: `[u32 LE frame_len][frame_data]` repeated.
/// Each `frame_data` is a raw Opus packet. Validation:
///   1. Parse the full framing to detect truncation or corrupt length prefixes.
///   2. Decode the first 16 packets with libopus to verify audio data integrity.
pub async fn validate_opus(cache: &FileAudioCache) -> Result<ActionMetrics> {
    let started = Instant::now();
    let mut processing_count = 0u64;
    let mut evict_count = 0u64;

    let entries = cache.db().get_all_entries().await?;

    for entry in entries {
        let Some(opus_path) = entry.opus_path else { continue };
        if entry.is_downloading {
            continue;
        }

        processing_count += 1;
        let path = PathBuf::from(&opus_path);

        let item_started = Instant::now();
        let is_valid = tokio::task::spawn_blocking(move || probe_opus_file(&path))
            .await
            .unwrap_or(false);

        let probe_ms = item_started.elapsed().as_millis();

        if !is_valid {
            let tap_id = TapId(entry.tap_id);
            let key: AudioCacheItemKey = match serde_json::from_str(&entry.cache_key) {
                Ok(k) => k,
                Err(e) => {
                    tracing::warn!(%e, tap_id = %tap_id.0, "failed to deserialize cache key, skipping");
                    continue;
                }
            };
            tracing::warn!(tap_id = %tap_id.0, opus_path, probe_ms, "corrupt/unreadable .opus file, evicting");
            match cache.delete(&tap_id, &key).await {
                Ok(()) => evict_count += 1,
                Err(e) => tracing::warn!(%e, tap_id = %tap_id.0, "failed to delete corrupt entry"),
            }
        }
    }

    Ok(ActionMetrics {
        action: "validate",
        processing_count,
        evict_count,
        processing_time_ms: started.elapsed().as_millis() as u64,
    })
}

/// Validate a custom-framed Opus cache file.
///
/// Returns `true` if the file can be fully framing-parsed and the first 16 packets
/// decode successfully with libopus.
fn probe_opus_file(path: &std::path::Path) -> bool {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(%e, path = %path.display(), "cannot open .opus file for validation");
            return false;
        }
    };

    let mut reader = BufReader::new(file);

    // Create a stereo 48 kHz Opus decoder; Opus self-describes channels in the TOC byte,
    // so the channel count here only affects the output buffer size.
    let decoder = match opus::Decoder::new(48000, opus::Channels::Stereo) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(?e, "failed to create Opus decoder for validation");
            return false;
        }
    };
    let mut decoder = decoder;
    // PCM output buffer: 20 ms at 48 kHz stereo = 960 * 2 samples
    let mut pcm = vec![0i16; 960 * 2];

    let max_decode = 16usize;
    let mut decoded = 0usize;

    loop {
        let mut len_buf = [0u8; 4];
        match reader.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break, // EOF — framing OK
            Err(e) => {
                tracing::debug!(%e, "framing read error during validation");
                return false;
            }
        }

        let frame_len = u32::from_le_bytes(len_buf) as usize;
        // Sanity check: Opus packets are typically a few KB at most.
        if frame_len == 0 || frame_len > 256 * 1024 {
            tracing::debug!(frame_len, "implausible Opus frame length");
            return false;
        }

        let mut frame_data = vec![0u8; frame_len];
        if let Err(e) = reader.read_exact(&mut frame_data) {
            tracing::debug!(%e, "truncated frame during validation");
            return false;
        }

        if decoded < max_decode {
            match decoder.decode(&frame_data, &mut pcm, false) {
                Ok(_) => decoded += 1,
                Err(e) => {
                    tracing::debug!(?e, "Opus decode error during validation");
                    return false;
                }
            }
        }
    }

    true
}
