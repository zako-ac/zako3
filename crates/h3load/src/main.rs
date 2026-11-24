//! h3load — synthetic taphub client used by the H-3 autoscaling load test.
//!
//! Speaks the engine-side transport (protofish3 over QUIC) to taphub, asks for a
//! tap's audio, and drains the returned stream. Every request prints one CSV line:
//!
//!   ts_ms,label,worker,idx,ars_len,rtt_ms,first_frame_ms,end_ms,samples,frames,status,err
//!
//! rtt_ms is time from sending the request to taphub answering it (for a TTS tap
//! that covers the whole synthesis), end_ms is when the stream closed.
//!
//! `--retry-once 1` (the default) makes the client behave like the documented
//! contract requires of an audio client: a failed request is retried once. The
//! retry is printed as its own row with the label suffixed `-retry`, so the
//! failure and the recovery that followed it can be paired by `worker`+`idx`.
//! `--retry-once 0` gives the old single-shot behaviour.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use zako3_taphub_transport_client::{TransportClient, load_certs};
use zako3_types::{
    AudioCachePolicy, AudioCacheType, CachedAudioRequest,
    hq::{DiscordUserId, TapId},
};

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn arg(args: &[String], name: &str, default: &str) -> String {
    if let Some(i) = args.iter().position(|a| a == name) {
        args.get(i + 1)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    } else {
        default.to_string()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .ok();

    let args: Vec<String> = std::env::args().collect();
    let addr = arg(&args, "--addr", "zako3-taphub:4000");
    let sni = arg(&args, "--sni", "zako3-taphub");
    let cert = arg(&args, "--cert", "/certs/transport/ca-cert.pem");
    let tap_id = arg(&args, "--tap-id", "");
    let text = arg(&args, "--text", "안녕하세요.");
    let label = arg(&args, "--label", "load");
    let concurrency: usize = arg(&args, "--concurrency", "4").parse().unwrap_or(4);
    let duration: u64 = arg(&args, "--duration-secs", "60").parse().unwrap_or(60);
    let gap_ms: u64 = arg(&args, "--gap-ms", "0").parse().unwrap_or(0);
    let vary: bool = arg(&args, "--vary", "1") != "0";
    let retry_once: bool = arg(&args, "--retry-once", "1") != "0";

    let certs = load_certs(&cert).with_context(|| format!("load_certs {cert}"))?;
    let client = Arc::new(
        TransportClient::connect(
            "0.0.0.0:0".parse().unwrap(),
            &addr,
            sni,
            certs,
            Duration::from_secs(30),
        )
        .await
        .map_err(|e| anyhow::anyhow!("connect: {e}"))?,
    );

    eprintln!(
        "h3load: label={label} concurrency={concurrency} duration={duration}s gap={gap_ms}ms tap={tap_id}"
    );

    let deadline = Instant::now() + Duration::from_secs(duration);
    let mut handles = Vec::new();

    for worker in 0..concurrency {
        let client = client.clone();
        let tap_id = tap_id.clone();
        let text = text.clone();
        let label = label.clone();
        handles.push(tokio::spawn(async move {
            let mut idx: u64 = 0;
            while Instant::now() < deadline {
                idx += 1;
                let ars = if vary {
                    format!("{text} {idx}")
                } else {
                    text.clone()
                };
                let ars_len = ars.chars().count();
                let ok = run_one(&client, &tap_id, &ars, ars_len, &label, worker, idx).await;
                if !ok && retry_once {
                    // The contract is "a failed request is retried once". Give the
                    // hub a moment to recompute its routing set (the draining
                    // connection is removed at the same instant the failure is
                    // delivered), then try the same text exactly once more.
                    tokio::time::sleep(Duration::from_millis(150)).await;
                    let rlabel = format!("{label}-retry");
                    run_one(&client, &tap_id, &ars, ars_len, &rlabel, worker, idx).await;
                }
                if gap_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(gap_ms)).await;
                }
            }
        }));
    }

    for h in handles {
        let _ = h.await;
    }
    eprintln!("h3load: done");
    Ok(())
}

/// Send one request and print one CSV row. Returns true when it succeeded.
async fn run_one(
    client: &TransportClient,
    tap_id: &str,
    ars: &str,
    ars_len: usize,
    label: &str,
    worker: usize,
    idx: u64,
) -> bool {
    let req = CachedAudioRequest {
        tap_id: TapId(tap_id.to_string()),
        audio_request: ars.into(),
        cache_key: AudioCachePolicy {
            cache_type: AudioCacheType::None,
            ttl_seconds: None,
        },
        discord_user_id: DiscordUserId(0.to_string()),
        headers: HashMap::new(),
    };
    let ts = now_ms();
    let t0 = Instant::now();
    match client.request_audio(req).await {
        Ok(mut resp) => {
            let rtt = t0.elapsed().as_millis();
            let mut first = rtt;
            let mut frames = 0usize;
            let mut samples = 0usize;
            while let Some(frame) = resp.stream.recv().await {
                if frames == 0 {
                    first = t0.elapsed().as_millis();
                }
                frames += 1;
                samples += frame.len();
            }
            let end = t0.elapsed().as_millis();
            println!(
                "{ts},{label},{worker},{idx},{ars_len},{rtt},{first},{end},{samples},{frames},ok,"
            );
            true
        }
        Err(e) => {
            let rtt = t0.elapsed().as_millis();
            println!("{ts},{label},{worker},{idx},{ars_len},{rtt},{rtt},{rtt},0,0,err,{e}");
            false
        }
    }
}
