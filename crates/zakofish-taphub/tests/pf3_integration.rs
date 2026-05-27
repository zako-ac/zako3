//! End-to-end integration test for the pf3 transport path of `ZakofishHub`.
//!
//! Spins up a hub configured with only a pf3 server and a `ZakofishTapPf3`
//! client, then exercises an audio request that streams `Dual`-mode chunks
//! through the timestamp-prefix framing. Mirrors `tests/integration.rs` (the
//! pf2 counterpart).

use bytes::Bytes;
use protofish2::{Timestamp, TransferMode};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use zako3_types::AudioRequestString;
use zako3_types::hq::TapId;
use zakofish::ZakofishTapPf3;
use zakofish::tap::TapHandler;
use zakofish::types::message::{
    AttachedMetadata, AudioMetadataSuccessMessage, AudioRequestFailureMessage,
    AudioRequestSuccessMessage, TapClientHello, TapServerReject,
};
use zakofish::types::model::{AudioCachePolicy, AudioCacheType};
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

struct TestHubHandler {
    tap_connected: mpsc::Sender<u64>,
}

#[async_trait::async_trait]
impl HubHandler for TestHubHandler {
    async fn on_tap_authenticate(
        &self,
        connection_id: u64,
        _hello: TapClientHello,
    ) -> Result<(), TapServerReject> {
        let _ = self.tap_connected.send(connection_id).await;
        Ok(())
    }
    async fn on_tap_disconnected(&self, _tap_id: TapId, _connection_id: u64) {}
}

struct TestTapHandler;

#[async_trait::async_trait]
impl TapHandler for TestTapHandler {
    async fn handle_audio_request(
        &self,
        _ars: zakofish::types::model::AudioRequestString,
        _headers: HashMap<String, String>,
    ) -> Result<
        (
            AudioRequestSuccessMessage,
            mpsc::Receiver<(Timestamp, Bytes)>,
            TransferMode,
        ),
        AudioRequestFailureMessage,
    > {
        let success_msg = AudioRequestSuccessMessage {
            cache: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
            duration_secs: Some(10.0),
            metadatas: AttachedMetadata::Metadatas(vec![]),
        };

        let (tx, rx) = mpsc::channel::<(Timestamp, Bytes)>(10);
        tokio::spawn(async move {
            for i in 0..5u64 {
                let timestamp = Timestamp(i * 100);
                let chunk = Bytes::from(format!("chunk {}", i));
                if tx.send((timestamp, chunk)).await.is_err() {
                    break;
                }
            }
        });

        Ok((success_msg, rx, TransferMode::Dual))
    }

    async fn handle_audio_metadata_request(
        &self,
        _ars: zakofish::types::model::AudioRequestString,
        _headers: HashMap<String, String>,
    ) -> Result<AudioMetadataSuccessMessage, AudioRequestFailureMessage> {
        Err(AudioRequestFailureMessage {
            reason: "Not found".to_string(),
            try_others: true,
        })
    }
}

#[tokio::test]
async fn test_zakofish_flow_pf3() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init()
        .ok();
    let _ = rustls::crypto::ring::default_provider().install_default();

    let (cert_chain, private_key) = gen_cert();

    let mut server_config = protofish3::ServerConfig::new(
        "127.0.0.1:0".parse().unwrap(),
        cert_chain.clone(),
        private_key,
    );
    server_config.protofish = zakofish::default_protofish3_config();

    let (tap_connected_tx, mut tap_connected_rx) = mpsc::channel(1);
    let hub_handler = Arc::new(TestHubHandler {
        tap_connected: tap_connected_tx,
    });

    let hub = Arc::new(ZakofishHub::new(None, Some(server_config), hub_handler).unwrap());
    let hub_clone = hub.clone();
    tokio::spawn(async move {
        let _ = hub_clone.run().await;
    });

    let local_addr = hub.local_addr_pf3().expect("pf3 bound").unwrap();

    let mut client_config = protofish3::ClientConfig::new("127.0.0.1:0".parse().unwrap());
    client_config.root_certificates = cert_chain;
    client_config.dangerously_skip_verification = true;
    client_config.protofish = zakofish::default_protofish3_config();

    let tap = Arc::new(ZakofishTapPf3::new(client_config).unwrap());
    let tap_handler = Arc::new(TestTapHandler);

    let tap_id_wire = zakofish::types::TapId("343456".to_string());
    let tap_id = TapId("343456".to_string());

    let hello_info = TapClientHello {
        tap_id: tap_id_wire,
        friendly_name: "Test Pf3 Tap".to_string(),
        api_token: "secret".to_string(),
        selection_weight: 1.0,
    };

    let tap_clone = tap.clone();
    tokio::spawn(async move {
        let _ = tap_clone
            .connect_and_run(local_addr, "localhost", hello_info, tap_handler)
            .await;
    });

    let connection_id = tap_connected_rx
        .recv()
        .await
        .expect("Failed to get connection_id");

    tokio::time::sleep(Duration::from_millis(100)).await;

    let ars = AudioRequestString::from("test:audio".to_string());
    let (success_msg, rel, mut unrel) = hub
        .request_audio(tap_id, connection_id, ars, HashMap::new())
        .await
        .expect("Failed to request audio");

    assert_eq!(success_msg.duration_secs, Some(10.0));

    let mut rel = rel.expect("pf3 Dual should yield a reliable stream");

    // Collect from both halves concurrently.
    let rel_task = tokio::spawn(async move {
        let mut out = Vec::new();
        while let Some(chunk) = rel.recv().await {
            out.push(chunk);
        }
        out
    });
    let unrel_task = tokio::spawn(async move {
        let mut out = Vec::new();
        while let Some((ts, chunk)) = unrel.recv().await {
            out.push((ts, chunk));
        }
        out
    });

    let rel_chunks = rel_task.await.unwrap();
    let unrel_chunks = unrel_task.await.unwrap();

    assert_eq!(rel_chunks.len(), 5);
    for (i, chunk) in rel_chunks.iter().enumerate() {
        assert_eq!(*chunk, Bytes::from(format!("chunk {}", i)));
    }

    assert_eq!(unrel_chunks.len(), 5);
    let mut sorted = unrel_chunks.clone();

    println!("sorted");
    sorted.sort_by_key(|(ts, _)| ts.0);
    for (i, (ts, chunk)) in sorted.iter().enumerate() {
        assert_eq!(ts.0, (i as u64) * 100);
        assert_eq!(*chunk, Bytes::from(format!("chunk {}", i)));
    }
}
