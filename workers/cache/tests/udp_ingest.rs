//! A preload arriving over UDP, with no audio engine anywhere in the path.
//!
//! Drives the real components — a real protofish4 sender against a real bound
//! receiver, through the real HTTP routes — because the parts worth checking
//! here are the orderings between them: armed before the reply is written,
//! committed only once both the audio and the metadata have landed, and
//! collected rather than left shadowing a cache key when the metadata never
//! comes.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use zako3_cache::server::{self, AppState, WarmupState};
use zako3_cache_client::{CreateIngestReq, FinalizeIngestReq, IngestCreatedResp};
use zako3_preload_cache::{AudioCache, AudioPreload, FileAudioCache};
use zako3_types::cache::{AudioCacheItem, AudioCacheItemKey};
use zako3_types::hq::TapId;
use zako3_types::{AudioCachePolicy, AudioCacheType, AudioMetadata};

const TAP: &str = "tap-udp";
const KEY: &str = "song-1";

struct Harness {
    state: AppState,
    router: Router,
    ingest_addr: std::net::SocketAddr,
    _dir: tempfile::TempDir,
}

async fn harness(max_sessions: usize) -> Harness {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = Arc::new(
        FileAudioCache::open_empty(dir.path().to_path_buf(), None)
            .await
            .expect("open cache"),
    );
    let preload = Arc::new(AudioPreload::new(dir.path().to_path_buf(), None));
    let ingest = server::ingest::start("127.0.0.1:0".parse().unwrap(), max_sessions)
        .await
        .expect("bind ingest");
    let ingest_addr = ingest.endpoint.local_addr().expect("local addr");

    let state = AppState::new(cache, preload, None, Arc::new(WarmupState::new()))
        .with_ingest(ingest);
    let router = server::build(state.clone());

    Harness { state, router, ingest_addr, _dir: dir }
}

fn cache_item() -> AudioCacheItem {
    AudioCacheItem {
        key: AudioCacheItemKey::CacheKey(KEY.to_string()),
        tap_id: TapId(TAP.to_string()),
        expire_at: None,
    }
}

fn policy() -> AudioCachePolicy {
    AudioCachePolicy {
        cache_type: AudioCacheType::CacheKey(KEY.to_string()),
        ttl_seconds: None,
    }
}

async fn post_json<T: serde::Serialize>(
    router: &Router,
    path: &str,
    body: &T,
) -> (StatusCode, bytes::Bytes) {
    let res = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(body).expect("encode")))
                .expect("build request"),
        )
        .await
        .expect("route request");
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("read body");
    (status, bytes)
}

async fn open_ingest(h: &Harness) -> IngestCreatedResp {
    let (status, body) = post_json(
        &h.router,
        "/ingest",
        &CreateIngestReq {
            item: cache_item(),
            metadatas: Vec::new(),
            cache_key: policy(),
        },
    )
    .await;
    assert_eq!(status, StatusCode::OK, "POST /ingest");
    serde_json::from_slice(&body).expect("decode ingest response")
}

/// Opus-shaped payloads. Nothing decodes them here; what matters is that the
/// exact bytes come back out in the exact order.
fn frames(n: usize) -> Vec<(protofish4::TimestampMs, Vec<u8>)> {
    (0..n)
        .map(|i| {
            let ts = protofish4::TimestampMs(i as u64 * 20);
            let payload: Vec<u8> = (0..40u8).map(|b| b.wrapping_add(i as u8)).collect();
            (ts, payload)
        })
        .collect()
}

fn payloads(frames: &[(protofish4::TimestampMs, Vec<u8>)]) -> Vec<Vec<u8>> {
    frames.iter().map(|(_, p)| p.clone()).collect()
}

/// Read the committed track back the way `GET /stream` does, so the framing on
/// disk is checked by the code that has to parse it rather than by an
/// assumption restated here.
async fn read_back(state: &AppState) -> Vec<Vec<u8>> {
    let mut reader = state
        .cache
        .open_reader(&TapId(TAP.to_string()), &AudioCacheItemKey::CacheKey(KEY.to_string()))
        .await
        .expect("committed entry is readable");
    let mut out = Vec::new();
    loop {
        match reader.next_frame().await.expect("read frame") {
            zako3_preload_cache::NextFrame::Frame(f) => out.push(f.to_vec()),
            zako3_preload_cache::NextFrame::Pending => {
                tokio::time::sleep(Duration::from_millis(5)).await
            }
            zako3_preload_cache::NextFrame::Done => break,
        }
    }
    out
}

fn titles(metadatas: &[AudioMetadata]) -> Vec<String> {
    metadatas
        .iter()
        .filter_map(|m| match m {
            AudioMetadata::Title(t) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

async fn send(h: &Harness, ticket: &IngestCreatedResp, frames: Vec<(protofish4::TimestampMs, Vec<u8>)>) {
    let key = protofish4::SessionKey::from_bytes(&ticket.encryption_key).expect("key");
    protofish4::send_all(
        vec![h.ingest_addr.to_string()],
        protofish4::RequestId(ticket.request_id),
        key,
        protofish4::SenderConfig::default(),
        frames,
    )
    .await
    .expect("send transfer");
}

async fn wait_for_entry(state: &AppState) -> zako3_preload_cache::CacheEntry {
    for _ in 0..400 {
        if let Some(entry) = state
            .cache
            .get_entry(&TapId(TAP.to_string()), &AudioCacheItemKey::CacheKey(KEY.to_string()))
            .await
            && entry.has_audio()
        {
            return entry;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the entry never committed");
}

/// The whole point: a tap streams straight to the cache worker and the bytes
/// land in the cache unchanged.
#[tokio::test]
async fn a_udp_transfer_commits_the_exact_bytes_that_were_sent() {
    let h = harness(4).await;
    let ticket = open_ingest(&h).await;
    let sent = frames(30);

    send(&h, &ticket, sent.clone()).await;

    let (status, _) = post_json(
        &h.router,
        &format!("/ingest/{}/finalize", ticket.preload_id),
        &FinalizeIngestReq {
            metadatas: vec![AudioMetadata::Title("Song".into())],
            cache_key: policy(),
        },
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "POST finalize");

    let entry = wait_for_entry(&h.state).await;
    assert_eq!(
        titles(&entry.metadatas),
        vec!["Song".to_string()],
        "the metadata attached at finalize is what gets stored"
    );
    assert_eq!(
        read_back(&h.state).await,
        payloads(&sent),
        "every frame, in order, framed exactly once"
    );
}

/// The metadata arrives on a different transport than the audio, and either can
/// finish first. Finalizing before a single packet has been sent must still
/// commit, once the audio catches up.
#[tokio::test]
async fn finalizing_before_the_audio_arrives_still_commits() {
    let h = harness(4).await;
    let ticket = open_ingest(&h).await;

    let (status, _) = post_json(
        &h.router,
        &format!("/ingest/{}/finalize", ticket.preload_id),
        &FinalizeIngestReq {
            metadatas: vec![AudioMetadata::Title("Early".into())],
            cache_key: policy(),
        },
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let sent = frames(5);
    send(&h, &ticket, sent.clone()).await;

    let entry = wait_for_entry(&h.state).await;
    assert_eq!(titles(&entry.metadatas), vec!["Early".to_string()]);
    assert_eq!(read_back(&h.state).await, payloads(&sent));
}

/// A complete transfer whose metadata never arrives must not commit: an entry
/// with no metadata reads back later as a track with no title rather than as a
/// failure, which is the sort of loss nobody notices for months.
#[tokio::test]
async fn audio_alone_does_not_commit() {
    let h = harness(4).await;
    let ticket = open_ingest(&h).await;

    send(&h, &ticket, frames(5)).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert!(
        h.state.sessions.contains_key(&ticket.preload_id),
        "the session must stay open, waiting for its metadata"
    );
    assert!(
        h.state
            .cache
            .get_entry(&TapId(TAP.to_string()), &AudioCacheItemKey::CacheKey(KEY.to_string()))
            .await
            .is_none(),
        "nothing may be committed without metadata"
    );
}

/// The receiver is armed before `POST /ingest` answers, so a tap that starts
/// sending the instant it learns the address cannot lose its opening packets.
/// Asserted by sending with no delay at all after the response is read.
#[tokio::test]
async fn the_receiver_is_armed_before_the_ticket_is_handed_out() {
    let h = harness(4).await;
    let ticket = open_ingest(&h).await;
    let sent = frames(3);

    send(&h, &ticket, sent.clone()).await;
    post_json(
        &h.router,
        &format!("/ingest/{}/finalize", ticket.preload_id),
        &FinalizeIngestReq { metadatas: vec![], cache_key: policy() },
    )
    .await;

    let _ = wait_for_entry(&h.state).await;
    assert_eq!(
        read_back(&h.state).await,
        payloads(&sent),
        "the first frame is not lost"
    );
}

/// Each transfer holds a reorder window, so the slot count is a memory bound
/// and has to be refused rather than queued.
#[tokio::test]
async fn ingest_slots_are_bounded() {
    let h = harness(1).await;
    let _first = open_ingest(&h).await;

    let (status, _) = post_json(
        &h.router,
        "/ingest",
        &CreateIngestReq {
            item: cache_item(),
            metadatas: Vec::new(),
            cache_key: policy(),
        },
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

/// `finalize` exists for UDP ingest only. An HTTP upload settles its metadata
/// when the session opens, and must not have it rewritten by a stray call.
#[tokio::test]
async fn finalize_is_refused_for_an_http_preload() {
    let h = harness(4).await;
    let preload_id = zako3_cache::server::preload::open_session(
        &h.state,
        zako3_cache_client::CreatePreloadReq {
            item: cache_item(),
            metadatas: vec![AudioMetadata::Title("Real".into())],
            cache_key: policy(),
        },
    )
    .expect("open http session");

    let (status, _) = post_json(
        &h.router,
        &format!("/ingest/{}/finalize", preload_id.0),
        &FinalizeIngestReq { metadatas: vec![], cache_key: policy() },
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
