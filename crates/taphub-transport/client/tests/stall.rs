//! A track that stalls must end, and ending it must not take its connection —
//! and every other track multiplexed over it — down with it.
//!
//! The stall is injected the way the audit's timeline describes it: the tap
//! sends a few frames, then goes quiet without ending the transfer. Before
//! P0-1 the pump kept the task alive after the consumer gave up, so the PCM
//! sender was never dropped and the mixer never ended the track.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rcgen::generate_simple_self_signed;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::sync::mpsc;

use zako3_opus_jitter::JitterConfig;
use zako3_taphub_transport_client::TransportClient;
use zako3_taphub_transport_server::{TapHubBridgeHandler, Timestamp, TransportServer};
use zako3_types::{
    AudioCachePolicy, AudioCacheType, AudioMetaResponse, AudioMetadata, AudioRequest,
    CachedAudioRequest, TapHubError,
};

const FRAME_MS: u64 = 20;
const SAMPLES_PER_FRAME: usize = 960; // 20 ms at 48 kHz

/// Encode `n` frames of a quiet tone: real Opus, so the jitter buffer decodes
/// rather than failing the track on a bad packet.
fn opus_frames(n: usize) -> Vec<Vec<u8>> {
    let mut enc = opus::Encoder::new(48_000, opus::Channels::Stereo, opus::Application::Audio)
        .expect("encoder");
    let mut phase = 0f32;
    (0..n)
        .map(|_| {
            let mut pcm = Vec::with_capacity(SAMPLES_PER_FRAME * 2);
            for _ in 0..SAMPLES_PER_FRAME {
                let s = (phase * std::f32::consts::TAU).sin() * 0.1;
                pcm.push(s);
                pcm.push(s);
                phase = (phase + 440.0 / 48_000.0) % 1.0;
            }
            enc.encode_vec_float(&pcm, 4000).expect("encode")
        })
        .collect()
}

fn audio_meta(cache_key: &str) -> AudioMetaResponse {
    AudioMetaResponse {
        cache_key: AudioCachePolicy {
            cache_type: AudioCacheType::CacheKey(cache_key.to_string()),
            ttl_seconds: None,
        },
        metadatas: vec![AudioMetadata::Title("Stall Test".to_string())],
        base_volume: 1.0,
    }
}

fn cached_request() -> CachedAudioRequest {
    CachedAudioRequest {
        tap_id: zako3_types::hq::TapId("test_tap".to_string()),
        audio_request: "yt:stall".to_string().into(),
        cache_key: AudioCachePolicy {
            cache_type: AudioCacheType::CacheKey("stall-key".to_string()),
            ttl_seconds: None,
        },
        discord_user_id: "123".to_string().into(),
        headers: HashMap::new(),
    }
}

fn meta_request() -> AudioRequest {
    AudioRequest {
        tap_id: zako3_types::hq::TapId("test_tap".to_string()),
        request: "yt:meta".to_string().into(),
        discord_user_id: "123".to_string().into(),
        headers: HashMap::new(),
    }
}

/// A tap that sends `frames` frames and then goes quiet without ending the
/// transfer — a wedge upstream, not the end of the stream. Holding `tx` is what
/// keeps the transfer open; dropping it would make the case an EOF instead.
struct SilentTap {
    frames: usize,
}

#[async_trait::async_trait]
impl TapHubBridgeHandler for SilentTap {
    async fn handle_request_audio(
        &self,
        req: CachedAudioRequest,
        _headers: HashMap<String, String>,
    ) -> Result<(AudioMetaResponse, mpsc::Receiver<(Timestamp, bytes::Bytes)>), TapHubError> {
        let (tx, rx) = mpsc::channel(16);
        let frames = opus_frames(self.frames);
        tokio::spawn(async move {
            for (i, frame) in frames.into_iter().enumerate() {
                let ts = Timestamp(i as u64 * FRAME_MS);
                if tx.send((ts, bytes::Bytes::from(frame))).await.is_err() {
                    return;
                }
            }
            // The tap has nothing more to say and never ends the transfer.
            std::future::pending::<()>().await;
        });

        let mut meta = audio_meta("stall-key");
        meta.cache_key = req.cache_key;
        Ok((meta, rx))
    }

    async fn handle_preload_audio(
        &self,
        _req: CachedAudioRequest,
        _headers: HashMap<String, String>,
    ) -> Result<AudioMetaResponse, TapHubError> {
        Err(TapHubError::Internal("unused".to_string()))
    }

    async fn handle_request_audio_meta(
        &self,
        _req: AudioRequest,
        _headers: HashMap<String, String>,
    ) -> Result<AudioMetaResponse, TapHubError> {
        Ok(audio_meta("meta-key"))
    }

    async fn handle_invalidate_cache(
        &self,
        _req: CachedAudioRequest,
        _headers: HashMap<String, String>,
    ) -> Result<(), TapHubError> {
        Ok(())
    }
}

/// Answers instantly, except for the first two calls, which outlive any sane
/// request timeout. Two attempts are therefore lost to timeouts before the third
/// call is served.
struct SlowFirstCalls {
    calls: Arc<AtomicUsize>,
    stall_for: Duration,
}

#[async_trait::async_trait]
impl TapHubBridgeHandler for SlowFirstCalls {
    async fn handle_request_audio(
        &self,
        _req: CachedAudioRequest,
        _headers: HashMap<String, String>,
    ) -> Result<(AudioMetaResponse, mpsc::Receiver<(Timestamp, bytes::Bytes)>), TapHubError> {
        Err(TapHubError::Internal("unused".to_string()))
    }

    async fn handle_preload_audio(
        &self,
        _req: CachedAudioRequest,
        _headers: HashMap<String, String>,
    ) -> Result<AudioMetaResponse, TapHubError> {
        Err(TapHubError::Internal("unused".to_string()))
    }

    async fn handle_request_audio_meta(
        &self,
        _req: AudioRequest,
        _headers: HashMap<String, String>,
    ) -> Result<AudioMetaResponse, TapHubError> {
        if self.calls.fetch_add(1, Ordering::SeqCst) < 2 {
            tokio::time::sleep(self.stall_for).await;
        }
        Ok(audio_meta("answered-key"))
    }

    async fn handle_invalidate_cache(
        &self,
        _req: CachedAudioRequest,
        _headers: HashMap<String, String>,
    ) -> Result<(), TapHubError> {
        Ok(())
    }
}

fn generate_certs() -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
    let cert = generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_der = CertificateDer::from(cert.cert.der().to_vec());
    let key_der = PrivateKeyDer::try_from(cert.key_pair.serialize_der()).unwrap();
    (vec![cert_der], key_der)
}

async fn spawn_server(
    handler: Arc<dyn TapHubBridgeHandler>,
) -> (std::net::SocketAddr, Vec<CertificateDer<'static>>) {
    let (server_certs, server_key) = generate_certs();
    let client_certs = server_certs.clone();

    let mut server = TransportServer::new(
        "127.0.0.1:0".parse().unwrap(),
        server_certs,
        server_key,
        handler,
    )
    .expect("server");
    let addr = server.local_addr().expect("local addr");
    tokio::spawn(async move {
        server.run().await;
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    (addr, client_certs)
}

/// The regression test for the audit's permanent stop: a source that goes quiet
/// mid-stream must not leave the track alive forever.
///
/// It proves three things at once, which is what the old `tokio::join!` broke:
/// (a) the task ends — the `recv` loop below terminates rather than hanging,
/// (b) `tx` is dropped — `recv` returns `None` while this test still holds the
///     receiver, so nothing but the task ending can have closed it, and
/// (c) `reset` is not called — the pooled connection survives, and the client
///     still serves a request afterwards.
#[tokio::test]
async fn stalled_track_ends_and_keeps_its_connection() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let (addr, certs) = spawn_server(Arc::new(SilentTap { frames: 6 })).await;
    let client = TransportClient::connect(
        "127.0.0.1:0".parse().unwrap(),
        &addr.to_string(),
        "localhost".to_string(),
        certs,
        Duration::from_secs(5),
    )
    .await
    .expect("client")
    .with_jitter_config(JitterConfig {
        stall_timeout: Duration::from_millis(300),
        ..Default::default()
    });

    let started = Instant::now();
    let mut stream = client
        .request_audio(cached_request())
        .await
        .expect("request_audio")
        .stream;

    let played = tokio::time::timeout(Duration::from_secs(10), async move {
        let mut played = 0;
        while stream.recv().await.is_some() {
            played += 1;
        }
        played
    })
    .await
    .expect("the track task must end after the stall and drop the PCM sender");
    let elapsed = started.elapsed();

    // At least every frame the tap sent must play before the stall. A late arrival
    // can make the buffer conceal a frame and push the count to 7, so this is a
    // floor rather than an equality.
    assert!(
        played >= 6,
        "every frame the tap sent should play before the stall, got {played}"
    );
    // A stalled source waits the stall timeout out; an undecodable one ends at
    // once. This distinguishes the path the fix is about from a decode failure.
    assert!(
        elapsed >= Duration::from_millis(250),
        "track ended in {elapsed:?}, too fast to have waited out the 300ms stall"
    );
    assert_eq!(
        client.resets_total(),
        0,
        "a stalled track must not reset the pooled connection"
    );

    // The connection the track streamed over is still up. The pool round-robins per
    // attempt, so a single request would land on the *next* slot and prove nothing
    // about the one that carried the track; POOL_SIZE requests cover every slot,
    // including that one.
    for _ in 0..4 {
        let meta = client
            .request_audio_meta(meta_request())
            .await
            .expect("the client must still serve requests after a track stalled");
        assert!(
            matches!(meta.cache_key.cache_type, AudioCacheType::CacheKey(ref k) if k == "meta-key")
        );
    }
}

/// A per-request timeout abandons only that request's channel. Resetting the
/// connection instead would cut every other track multiplexed over it, which is
/// the side effect the audit found in production.
#[tokio::test]
async fn request_timeout_does_not_reset_the_connection() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let calls = Arc::new(AtomicUsize::new(0));
    let (addr, certs) = spawn_server(Arc::new(SlowFirstCalls {
        calls: calls.clone(),
        stall_for: Duration::from_secs(30),
    }))
    .await;

    let client = TransportClient::connect(
        "127.0.0.1:0".parse().unwrap(),
        &addr.to_string(),
        "localhost".to_string(),
        certs,
        Duration::from_millis(500),
    )
    .await
    .expect("client");

    let err = client
        .request_audio_meta(meta_request())
        .await
        .expect_err("both attempts must time out against a server that never answers");
    assert!(
        format!("{err:?}").contains("timed out"),
        "expected a timeout error, got {err:?}"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "the request must be retried exactly once"
    );
    assert_eq!(
        client.resets_total(),
        0,
        "a timed-out request must not reset the pooled connection"
    );

    // And the client is still healthy: the pool was never torn down.
    let meta = client
        .request_audio_meta(meta_request())
        .await
        .expect("the pool must still serve requests after timeouts");
    assert!(
        matches!(meta.cache_key.cache_type, AudioCacheType::CacheKey(ref k) if k == "answered-key")
    );
}
