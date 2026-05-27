use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{SampleFormat, WavSpec, WavWriter};
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer as _, Split as _};
use tokio::sync::mpsc::Receiver;
use zako3_taphub_transport_client::{TransportClient, load_certs};
use zako3_types::{
    AudioCachePolicy, AudioCacheType, AudioMetadata, CachedAudioRequest,
    hq::{DiscordUserId, TapId},
};

use crate::services::taphub::cli::{TaphubCommands, TaphubSubcommand};

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;

pub async fn handle_command(cmd: TaphubCommands) -> Result<()> {
    match cmd.command {
        TaphubSubcommand::Request {
            server_addr,
            server_name,
            cert_file,
            tap_id,
            discord_user_id,
            ars,
            output,
            play,
        } => {
            if output.is_none() && !play {
                bail!("Must specify either --output <PATH> or --play");
            }

            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

            let certs = load_certs(&cert_file)
                .with_context(|| format!("Failed to load certs from {:?}", cert_file))?;

            println!("Connecting to taphub at {} (SNI: {})...", server_addr, server_name);
            let client = TransportClient::connect(
                "0.0.0.0:0".parse().unwrap(),
                &server_addr,
                server_name,
                certs,
            )
            .await
            .map_err(|e| anyhow!("Failed to connect to taphub: {}", e))?;

            let request = CachedAudioRequest {
                tap_id: TapId(tap_id),
                audio_request: ars.into(),
                cache_key: AudioCachePolicy {
                    cache_type: AudioCacheType::None,
                    ttl_seconds: None,
                },
                discord_user_id: DiscordUserId(discord_user_id),
                headers: HashMap::new(),
            };

            println!("Sending request_audio...");
            let resp = client
                .request_audio(request)
                .await
                .map_err(|e| anyhow!("request_audio failed: {}", e))?;

            print_metadata(&resp.metadatas);

            if let Some(path) = output {
                write_wav(&path, resp.stream).await?;
            } else {
                play_live(resp.stream).await?;
            }

            Ok(())
        }
    }
}

fn print_metadata(metadatas: &[AudioMetadata]) {
    println!("Metadata:");
    if metadatas.is_empty() {
        println!("  (none)");
        return;
    }
    for m in metadatas {
        println!("  - {:?}", m);
    }
}

async fn write_wav(path: &Path, mut stream: Receiver<Vec<f32>>) -> Result<()> {
    let spec = WavSpec {
        channels: CHANNELS,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec)
        .with_context(|| format!("Failed to create WAV file at {:?}", path))?;

    println!("Writing WAV to {:?} (Ctrl-C to stop)...", path);

    let mut total_samples: u64 = 0;
    loop {
        tokio::select! {
            biased;
            res = tokio::signal::ctrl_c() => {
                if let Err(e) = res {
                    eprintln!("Ctrl-C handler failed: {}", e);
                }
                println!("\nCtrl-C received, finalizing WAV...");
                break;
            }
            frame = stream.recv() => {
                let Some(frame) = frame else {
                    println!("Stream ended.");
                    break;
                };
                for sample in &frame {
                    let clamped = sample.clamp(-1.0, 1.0);
                    let s_i16 = (clamped * i16::MAX as f32) as i16;
                    writer.write_sample(s_i16)
                        .context("Failed to write WAV sample")?;
                }
                total_samples += frame.len() as u64;
            }
        }
    }

    writer.finalize().context("Failed to finalize WAV file")?;
    let frames = total_samples / CHANNELS as u64;
    println!(
        "Wrote {} samples ({} frames, ~{:.2}s) to {:?}",
        total_samples,
        frames,
        frames as f64 / SAMPLE_RATE as f64,
        path
    );
    Ok(())
}

async fn play_live(mut stream: Receiver<Vec<f32>>) -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("No default audio output device available")?;
    #[allow(deprecated)]
    if let Ok(name) = device.name() {
        println!("Using output device: {}", name);
    }

    let config = cpal::StreamConfig {
        channels: CHANNELS,
        sample_rate: SAMPLE_RATE,
        buffer_size: cpal::BufferSize::Default,
    };

    // 1 second buffer of stereo audio
    let rb = HeapRb::<f32>::new((SAMPLE_RATE as usize) * (CHANNELS as usize));
    let (mut prod, mut cons) = rb.split();

    let err_fn = |err| eprintln!("cpal stream error: {}", err);
    let cpal_stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                let filled = cons.pop_slice(data);
                for s in data[filled..].iter_mut() {
                    *s = 0.0;
                }
            },
            err_fn,
            None,
        )
        .context("Failed to build cpal output stream")?;
    cpal_stream.play().context("Failed to start playback")?;

    println!("Playing (Ctrl-C to stop)...");

    loop {
        tokio::select! {
            biased;
            res = tokio::signal::ctrl_c() => {
                if let Err(e) = res {
                    eprintln!("Ctrl-C handler failed: {}", e);
                }
                println!("\nCtrl-C received, stopping...");
                break;
            }
            frame = stream.recv() => {
                let Some(frame) = frame else {
                    println!("Stream ended.");
                    // Give cpal a moment to drain the ring buffer.
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    break;
                };
                push_samples_blocking(&mut prod, &frame).await;
            }
        }
    }

    drop(cpal_stream);
    Ok(())
}

/// Push samples into the ring buffer, yielding while the buffer is full so the
/// cpal callback can drain. The producer is single-consumer; capacity bounds the
/// backpressure window between the tokio task and the audio thread.
async fn push_samples_blocking<P>(prod: &mut P, mut frame: &[f32])
where
    P: ringbuf::traits::Producer<Item = f32>,
{
    while !frame.is_empty() {
        let n = prod.push_slice(frame);
        if n == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            continue;
        }
        frame = &frame[n..];
    }
}

