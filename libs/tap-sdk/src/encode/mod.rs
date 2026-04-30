#[cfg(feature = "auto-encode")]
use crate::stream::AudioStreamSender;

/// High-level helper: pipe an audio file through ffmpeg and stream Opus frames.
///
/// Uses `ffmpeg -c:a copy` to remux to OGG/Opus (codec-copy, no re-encode).
/// Requires ffmpeg to be installed and available on PATH.
///
/// # Example
/// ```ignore
/// let buf = std::fs::read("audio.webm")?;
/// let cursor = std::io::Cursor::new(buf);
/// decode_and_stream(cursor, stream).await?;
/// ```
#[cfg(feature = "auto-encode")]
pub async fn decode_and_stream(
    reader: std::io::Cursor<Vec<u8>>,
    stream: AudioStreamSender,
) -> Result<(), EncodeError> {
    use std::process::Stdio;
    use tokio::io::AsyncWriteExt;
    use tokio::process::Command;
    use tokio_stream::StreamExt;

    // Spawn ffmpeg: pipe:0 (input) → ogg/opus (codec copy) → pipe:1 (output)
    let mut ffmpeg = Command::new("ffmpeg")
        //.args(["-v", "quiet", "-i", "pipe:0", "-vn", "-c:a", "copy", "-f", "ogg", "pipe:1"])
        .args([
            "-v", "quiet", "-i", "pipe:0", "-vn", "-c:a", "libopus", "-f", "ogg", "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(EncodeError::Spawn)?;

    let mut ffmpeg_in = ffmpeg.stdin.take().unwrap();
    let ffmpeg_out = ffmpeg.stdout.take().unwrap();

    // Write input bytes to ffmpeg stdin in a separate task
    let data = reader.into_inner();
    tokio::spawn(async move {
        ffmpeg_in.write_all(&data).await.ok();
    });

    // Read OGG packets from ffmpeg stdout
    let mut ogg_reader = ogg::reading::async_api::PacketReader::new(ffmpeg_out);
    let mut frame_index = 0u64;

    println!("121");

    while let Some(result) = ogg_reader.next().await {
        println!("got packet");

        match result {
            Ok(packet) => {
                // Skip OGG metadata packets
                if packet.data.starts_with(b"OpusHead") || packet.data.starts_with(b"OpusTags") {
                    continue;
                }
                let data = bytes::Bytes::copy_from_slice(&packet.data);
                println!("sending packet with {} bytes", data.len());
                if !stream.send_opus_frame(frame_index, data).await {
                    break; // Hub disconnected
                }
                println!("sent packet with index {}", frame_index);
                frame_index += 1;
            }
            Err(e) => {
                tracing::warn!("ogg packet read error: {}", e);
                break;
            }
        }
        println!("sent packet");
    }

    //ffmpeg.wait().await.ok();
    Ok(())
}

/// Error type for ffmpeg-based audio streaming.
#[cfg(feature = "auto-encode")]
#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("failed to spawn ffmpeg: {0}")]
    Spawn(#[from] std::io::Error),
}
