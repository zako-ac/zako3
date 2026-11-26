//! The hub closing a connection that stopped answering, and the tap coming back.
//!
//! This is the recovery S2b needed. On 2026-11-27 00:25 a taphub restart closed
//! a wedged connection and the tap reconnected and answered again inside 90
//! seconds — nothing about the tap's process needed touching. The hub-side close
//! API is that restart narrowed to one connection, and this test is the
//! contract: the hub can close a live connection, the peer sees the close, and
//! the peer's own reconnect path puts it back without anyone restarting a pod.
//!
//! What is asserted here is transport-level, deliberately. The *decision* to
//! close (`silent 300s AND strikes > 0`) is a unit test in `zako3-taphub-core`
//! where a registry can be built without a socket; this file proves that acting
//! on that decision actually recovers a tap.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::sync::mpsc;
use zako3_types::hq::TapId;
use zakofish::tap::TapHandler;
use zakofish::tap_pf3::{DrainConfig, ZakofishTapPf3};
use zakofish::types::message::{
    AudioMetadataSuccessMessage, AudioRequestFailureMessage, AudioRequestSuccessMessage,
    TapClientHello, TapServerReject,
};
use zakofish::types::model::AudioRequestString;
use zakofish::{Timestamp, TransferMode};
use zakofish_taphub::hub::{HubHandler, ZakofishHub};

/// The code the hub sends when it self-heals a connection. Kept as a literal
/// here on purpose: it is the wire value, and a test that reads it from the
/// implementation would not notice the implementation changing it.
const SELF_HEAL_CLOSE_CODE: u64 = 4100;

fn gen_cert() -> (
    Vec<rustls::pki_types::CertificateDer<'static>>,
    rustls::pki_types::PrivateKeyDer<'static>,
) {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let der_cert = cert.cert.der().clone();
    let der_key = rustls::pki_types::PrivateKeyDer::try_from(cert.signing_key.serialize_der()).unwrap();
    (vec![der_cert], der_key)
}

#[derive(Default)]
struct RecordingHub {
    /// Every authentication, in order. A second entry is a reconnection.
    connected: Mutex<Vec<u64>>,
    /// connection id, when the hub saw the close, and the close code.
    disconnected: Mutex<Vec<(u64, Option<u64>)>>,
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
        self.disconnected.lock().unwrap().push((connection_id, close_code));
    }
}

/// Never asked anything in this test: the connection is what is under test, not
/// the handler.
struct IdleTap;

#[async_trait::async_trait]
impl TapHandler for IdleTap {
    async fn handle_audio_request(
        &self,
        _ars: AudioRequestString,
        _headers: HashMap<String, String>,
    ) -> Result<
        (
            AudioRequestSuccessMessage,
            mpsc::Receiver<(Timestamp, Bytes)>,
            TransferMode,
        ),
        AudioRequestFailureMessage,
    > {
        Err(AudioRequestFailureMessage {
            reason: "idle test tap".to_string(),
            try_others: true,
        })
    }

    async fn handle_audio_metadata_request(
        &self,
        _ars: AudioRequestString,
        _headers: HashMap<String, String>,
    ) -> Result<AudioMetadataSuccessMessage, AudioRequestFailureMessage> {
        Err(AudioRequestFailureMessage {
            reason: "idle test tap".to_string(),
            try_others: true,
        })
    }
}

const TAP_ID: &str = "424242";

struct Fixture {
    hub: Arc<ZakofishHub>,
    recorder: Arc<RecordingHub>,
}

fn fixture() -> Fixture {
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
    tokio::spawn(async move {
        let hello = TapClientHello {
            tap_id: zakofish::types::TapId(TAP_ID.to_string()),
            friendly_name: "Self-heal Test Tap".to_string(),
            api_token: "secret".to_string(),
            selection_weight: 1.0,
        };
        // No signal handler: this test drives the close from the hub side, and a
        // SIGTERM handler in a test process would swallow the harness's signals.
        let drain = DrainConfig {
            timeout: Duration::from_secs(2),
            handle_signals: false,
        };
        // `connect_and_run_with` reconnects on its own — that loop is the thing
        // under test, so it must be the real one and not a hand-rolled retry.
        let _ = tap
            .connect_and_run_with(&addr, "localhost", hello, Arc::new(IdleTap), drain)
            .await;
    });

    Fixture { hub, recorder }
}

/// Wait for the `n`th authentication (1-based) and return its connection id.
async fn wait_for_connection(recorder: &RecordingHub, n: usize) -> u64 {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(id) = recorder.connected.lock().unwrap().get(n - 1).copied() {
            return id;
        }
        assert!(
            Instant::now() < deadline,
            "the tap never authenticated {} time(s); saw {:?}",
            n,
            recorder.connected.lock().unwrap()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// The hub can close a connection it holds, with a code of its own, and the hub
/// sees that close as its own doing rather than as a lost peer.
#[tokio::test]
async fn the_hub_closes_a_connection_it_holds() {
    let f = fixture();
    let connection_id = wait_for_connection(&f.recorder, 1).await;

    // The control chan is opened right after the handshake; let it settle so the
    // close is not racing the setup.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let tap_id = TapId(TAP_ID.to_string());
    let closed = f
        .hub
        .close_connection(&tap_id, connection_id, SELF_HEAL_CLOSE_CODE)
        .await;
    assert!(closed, "the hub did not find the session it just accepted");

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some((id, code)) = f.recorder.disconnected.lock().unwrap().first().copied() {
            assert_eq!(id, connection_id, "the wrong connection was closed");
            // The code we send goes on the wire *for the peer*; it is not
            // observable from this side. protofish3's driver records the terminal
            // code of its own loop and its `run()` overwrites that with `Some(0)`
            // when the loop exits cleanly after a local close. That is exactly
            // why taphub decides `reason=self_heal` from the entry's own
            // `self_healing` flag rather than from the close code, and why this
            // asserts the close happened instead of what this side thinks the
            // code was.
            assert!(
                matches!(code, Some(0) | None),
                "a locally initiated close must not surface as a peer code; saw {code:?}"
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the hub never saw the connection close"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Closing a connection the hub does not hold is a no-op, not an error: a sweep
/// can always race a normal teardown.
#[tokio::test]
async fn closing_an_unknown_connection_reports_nothing_to_close() {
    let f = fixture();
    let _connection_id = wait_for_connection(&f.recorder, 1).await;

    let closed = f
        .hub
        .close_connection(&TapId(TAP_ID.to_string()), 999_999, SELF_HEAL_CLOSE_CODE)
        .await;
    assert!(!closed, "an unknown connection id must not report a close");

    let other_tap = f
        .hub
        .close_connection(&TapId("nobody".to_string()), 1, SELF_HEAL_CLOSE_CODE)
        .await;
    assert!(!other_tap, "an unknown tap must not report a close");
}

/// The acceptance criterion, end to end: the hub closes a connection whose peer
/// has stopped answering, and the tap reconnects **on its own** — no pod
/// restart, no signal to the process, nothing but the close.
#[tokio::test]
async fn a_closed_connection_reconnects_itself() {
    let f = fixture();
    let first = wait_for_connection(&f.recorder, 1).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let closed_at = Instant::now();
    let closed = f
        .hub
        .close_connection(&TapId(TAP_ID.to_string()), first, SELF_HEAL_CLOSE_CODE)
        .await;
    assert!(closed);

    let second = wait_for_connection(&f.recorder, 2).await;
    let recovery = closed_at.elapsed();

    assert_ne!(
        second, first,
        "the reconnection reused the closed connection id"
    );
    assert!(
        recovery < Duration::from_secs(10),
        "the tap took {recovery:?} to reconnect; the measured recovery is ~90s in \
         production and the client's own backoff starts at 1s"
    );
}
