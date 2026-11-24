//! A tap leaving on purpose: the drain notice, and how fast the hub acts on it.
//!
//! The contract this covers is the scale-down one. A pod being deleted gets a
//! SIGTERM and a grace period; what the rest of the system needs is for the
//! hub to stop routing to it and to fail its in-flight requests *then*, rather
//! than when the QUIC connection finally gives up on a process that has stopped
//! reading. Both halves are asserted here in wall-clock terms, because "it
//! converges eventually" is exactly the failure being fixed.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::sync::mpsc;
use zako3_types::AudioRequestString;
use zako3_types::hq::TapId;
use zakofish::tap::TapHandler;
use zakofish::tap_pf3::{DrainConfig, ZakofishTapPf3};
use zakofish::types::message::{
    AttachedMetadata, AudioMetadataSuccessMessage, AudioRequestFailureMessage,
    AudioRequestSuccessMessage, TapClientHello, TapDraining, TapServerReject,
};
use zakofish::types::model::{AudioCachePolicy, AudioCacheType};
use zakofish::{Timestamp, TransferMode};
use zakofish_taphub::hub::{HubHandler, ZakofishHub};

fn gen_cert() -> (
    Vec<rustls::pki_types::CertificateDer<'static>>,
    rustls::pki_types::PrivateKeyDer<'static>,
) {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let der_cert = cert.cert.der().clone();
    let der_key =
        rustls::pki_types::PrivateKeyDer::try_from(cert.signing_key.serialize_der()).unwrap();
    (vec![der_cert], der_key)
}

#[derive(Default)]
struct RecordingHub {
    connected: Mutex<Vec<u64>>,
    draining: Mutex<Vec<(u64, TapDraining, Instant)>>,
    /// connection id, when the hub saw the close, and the close code it
    /// recorded — `Some(0)` is the frame a draining tap sends.
    disconnected: Mutex<Vec<(u64, Instant, Option<u64>)>>,
}

#[async_trait::async_trait]
impl HubHandler for RecordingHub {
    async fn on_tap_authenticate(
        &self,
        connection_id: u64,
        _hello: TapClientHello,
    ) -> Result<(), TapServerReject> {
        self.connected.lock().unwrap().push(connection_id);
        Ok(())
    }

    async fn on_tap_disconnected(
        &self,
        _tap_id: TapId,
        connection_id: u64,
        close_code: Option<u64>,
    ) {
        self.disconnected.lock().unwrap().push((
            connection_id,
            Instant::now(),
            close_code,
        ));
    }

    async fn on_tap_draining(&self, _tap_id: TapId, connection_id: u64, info: TapDraining) {
        self.draining
            .lock()
            .unwrap()
            .push((connection_id, info, Instant::now()));
    }
}

/// Answers immediately unless the request asks for `slow:`, in which case it
/// takes long enough to still be running when a drain starts.
struct QuickTap;

#[async_trait::async_trait]
impl TapHandler for QuickTap {
    async fn handle_audio_request(
        &self,
        ars: zakofish::types::model::AudioRequestString,
        _headers: HashMap<String, String>,
    ) -> Result<
        (
            AudioRequestSuccessMessage,
            mpsc::Receiver<(Timestamp, Bytes)>,
            TransferMode,
        ),
        AudioRequestFailureMessage,
    > {
        if ars.to_string().contains("slow") {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let success = AudioRequestSuccessMessage {
            cache: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
            duration_secs: Some(1.0),
            metadatas: AttachedMetadata::Metadatas(vec![]),
        };
        let (tx, rx) = mpsc::channel(1);
        tokio::spawn(async move {
            let _ = tx.send((Timestamp(0), Bytes::from_static(b"x"))).await;
        });
        Ok((success, rx, TransferMode::Dual))
    }

    async fn handle_audio_metadata_request(
        &self,
        _ars: zakofish::types::model::AudioRequestString,
        _headers: HashMap<String, String>,
    ) -> Result<AudioMetadataSuccessMessage, AudioRequestFailureMessage> {
        Err(AudioRequestFailureMessage {
            reason: "not found".to_string(),
            try_others: true,
        })
    }
}

struct Fixture {
    hub: Arc<ZakofishHub>,
    recorder: Arc<RecordingHub>,
    tap: Arc<ZakofishTapPf3>,
    /// Set once the tap's task has finished, i.e. the drain completed.
    tap_done: tokio::task::JoinHandle<()>,
}

fn fixture(drain_timeout: Duration) -> Fixture {
    let _ = rustls::crypto::ring::default_provider().install_default();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init()
        .ok();
    let (cert_chain, private_key) = gen_cert();

    let mut server_config = protofish3::ServerConfig::new(
        "127.0.0.1:0".parse().unwrap(),
        cert_chain.clone(),
        private_key,
    );
    server_config.protofish = zakofish::default_protofish3_config();

    let recorder = Arc::new(RecordingHub::default());
    let hub = Arc::new(ZakofishHub::new(server_config, recorder.clone()).unwrap());
    let hub_for_task = hub.clone();
    tokio::spawn(async move {
        let _ = hub_for_task.run().await;
    });

    let addr = hub.local_addr().unwrap().to_string();

    let mut client_config = protofish3::ClientConfig::new("127.0.0.1:0".parse().unwrap());
    client_config.root_certificates = cert_chain;
    client_config.dangerously_skip_verification = true;
    client_config.protofish = zakofish::default_protofish3_config();

    let tap = Arc::new(ZakofishTapPf3::new(client_config).unwrap());
    let tap_for_task = tap.clone();

    // No signal handler: the test drives the drain itself, and a test process
    // that installed a SIGTERM handler would swallow signals meant for the
    // harness.
    let drain = DrainConfig {
        timeout: drain_timeout,
        handle_signals: false,
    };

    let tap_done = tokio::spawn(async move {
        let hello = TapClientHello {
            tap_id: zakofish::types::TapId("424242".to_string()),
            friendly_name: "Drain Test Tap".to_string(),
            api_token: "secret".to_string(),
            selection_weight: 1.0,
        };
        let _ = tap_for_task
            .connect_and_run_with(&addr, "localhost", hello, Arc::new(QuickTap), drain)
            .await;
    });

    Fixture {
        hub,
        recorder,
        tap,
        tap_done,
    }
}

async fn wait_for_connection(recorder: &RecordingHub) -> u64 {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(id) = recorder.connected.lock().unwrap().first().copied() {
            return id;
        }
        assert!(Instant::now() < deadline, "the tap never authenticated");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// The scale-down case, end to end: a tap says it is draining and the hub hears
/// it inside a second, without waiting for the connection to time out.
#[tokio::test]
async fn a_drain_notice_reaches_the_hub_inside_a_second() {
    let f = fixture(Duration::from_secs(2));
    let connection_id = wait_for_connection(&f.recorder).await;

    // Let the control chan finish opening; it is opened right after the
    // handshake and the notice has nowhere to go before then. The tap tolerates
    // it being absent, which is the case this sleep would otherwise hide.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let drain_started = Instant::now();
    f.tap.drain_now();

    let deadline = Duration::from_secs(1);
    loop {
        if let Some((id, info, at)) = f.recorder.draining.lock().unwrap().first().cloned() {
            assert_eq!(id, connection_id, "drain notice named the wrong connection");
            let latency = at.duration_since(drain_started);
            assert!(
                latency < deadline,
                "drain notice took {latency:?}, contract is under {deadline:?}"
            );
            // The tap's own timestamp is what the hub records against; it must
            // be a real clock reading, not a zero placeholder.
            assert!(
                info.started_at_unix_ms > 1_700_000_000_000,
                "drain notice carried no usable timestamp: {}",
                info.started_at_unix_ms
            );
            break;
        }
        assert!(
            drain_started.elapsed() < Duration::from_secs(5),
            "the hub never saw a drain notice"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// After the drain the tap closes its own connection, so the hub's teardown runs
/// too — the drain is a shortcut for the routing table, not a replacement for
/// the disconnect path.
#[tokio::test]
async fn a_drained_tap_closes_its_connection_and_stops() {
    let f = fixture(Duration::from_secs(2));
    let connection_id = wait_for_connection(&f.recorder).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let drain_started = Instant::now();
    f.tap.drain_now();

    tokio::time::timeout(Duration::from_secs(5), f.tap_done)
        .await
        .expect("the tap did not finish its drain")
        .expect("the tap task panicked");

    let disconnected = f.recorder.disconnected.lock().unwrap().clone();
    let (id, at, close_code) = disconnected
        .first()
        .cloned()
        .expect("the hub never saw the connection close");
    assert_eq!(id, connection_id);
    assert!(
        at.duration_since(drain_started) < Duration::from_secs(3),
        "the connection outlived the drain: {:?}",
        at.duration_since(drain_started)
    );
    // The close frame is the difference between this and a process that simply
    // died: without a code the hub cannot tell a departure from a lost
    // connection, and it will not retry the difference either.
    assert_eq!(
        close_code,
        Some(0),
        "the tap did not close with a close frame; the hub saw {close_code:?}"
    );
}

/// A request already running when the drain starts is reported in the notice:
/// the hub uses it to decide whether there is anything to wait for, so a zero
/// here would let the tap close on top of live work.
#[tokio::test]
async fn a_drain_reports_the_requests_it_is_still_serving() {
    let f = fixture(Duration::from_secs(5));
    let connection_id = wait_for_connection(&f.recorder).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // A request the tap is deliberately slow to answer, so it is genuinely
    // running when the drain starts. `slow:` is what `QuickTap` keys on.
    let tap_id = TapId("424242".to_string());
    let hub = f.hub.clone();
    let pending = tokio::spawn(async move {
        hub.request_audio(
            tap_id,
            connection_id,
            AudioRequestString::from("slow:audio".to_string()),
            HashMap::new(),
        )
        .await
    });

    // Give the request time to be accepted and counted.
    tokio::time::sleep(Duration::from_millis(250)).await;
    f.tap.drain_now();

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some((_, info, _)) = f.recorder.draining.lock().unwrap().first().cloned() {
            assert!(
                info.in_flight >= 1,
                "the drain notice said nothing was running while a request was"
            );
            break;
        }
        assert!(Instant::now() < deadline, "no drain notice");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let _ = pending.await;
}
