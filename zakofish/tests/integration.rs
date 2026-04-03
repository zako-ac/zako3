use bytes::Bytes;
use protofish2::Timestamp;
use protofish2::compression::CompressionType;
use protofish2::connection::{ClientConfig, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Notify, mpsc};
use zako3_types::AudioRequestString;
use zako3_types::hq::TapId;
use zako3_types::{AudioCachePolicy, AudioCacheType};
use zakofish::hub::{HubHandler, ZakofishHub};
use zakofish::tap::{TapHandler, ZakofishTap};
use zakofish::types::message::{
    AudioMetadataSuccessMessage, AudioRequestFailureMessage, AudioRequestSuccessMessage,
    TapClientHello, TapServerReject,
};

fn generate_cert() -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
    let subject_alt_names = vec!["localhost".to_string()];
    let cert = rcgen::generate_simple_self_signed(subject_alt_names).unwrap();
    let der_cert = cert.cert.der().to_vec();
    let der_key = cert.signing_key.serialize_der();
    (
        vec![CertificateDer::from(der_cert)],
        PrivateKeyDer::Pkcs8(der_key.into()),
    )
}

struct TestHubHandler {
    tap_connected: Arc<Notify>,
}

#[async_trait::async_trait]
impl HubHandler for TestHubHandler {
    async fn on_tap_authenticate(&self, _hello: TapClientHello) -> Result<(), TapServerReject> {
        self.tap_connected.notify_one();
        Ok(())
    }
    async fn on_tap_disconnected(&self, _tap_id: TapId) {}
}

struct TestTapHandler;

#[async_trait::async_trait]
impl TapHandler for TestTapHandler {
    async fn handle_audio_request(
        &self,
        _ars: AudioRequestString,
        _headers: HashMap<String, String>,
    ) -> Result<
        (
            AudioRequestSuccessMessage,
            mpsc::Receiver<(Timestamp, Bytes)>,
        ),
        AudioRequestFailureMessage,
    > {
        let success_msg = AudioRequestSuccessMessage {
            cache: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
            duration_secs: Some(10.0),
            metadatas: vec![],
        };

        let (tx, rx) = mpsc::channel(10);

        tokio::spawn(async move {
            for i in 0..5 {
                let timestamp = Timestamp(i as u64 * 100);
                let chunk = Bytes::from(format!("chunk {}", i));
                if tx.send((timestamp, chunk)).await.is_err() {
                    break;
                }
            }
        });

        Ok((success_msg, rx))
    }

    async fn handle_audio_metadata_request(
        &self,
        ars: AudioRequestString,
        _headers: HashMap<String, String>,
    ) -> Result<AudioMetadataSuccessMessage, AudioRequestFailureMessage> {
        if ars.to_string() == "test:metadata" {
            Ok(AudioMetadataSuccessMessage { metadatas: vec![] })
        } else {
            Err(AudioRequestFailureMessage {
                reason: "Not found".to_string(),
                try_others: true,
            })
        }
    }
}

#[tokio::test]
async fn test_zakofish_flow() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init()
        .ok();
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let (cert_chain, private_key) = generate_cert();

    let server_config = ServerConfig {
        bind_address: "127.0.0.1:0".parse().unwrap(),
        cert_chain: cert_chain.clone(),
        private_key,
        supported_compression_types: vec![CompressionType::None],
        keepalive_interval: Duration::from_secs(5),
        protofish_config: Default::default(),
    };

    let tap_connected = Arc::new(Notify::new());
    let hub_handler = Arc::new(TestHubHandler {
        tap_connected: tap_connected.clone(),
    });

    let hub = Arc::new(ZakofishHub::new(server_config, hub_handler).unwrap());
    let hub_clone = hub.clone();

    tokio::spawn(async move {
        hub_clone.run().await.unwrap();
    });

    let local_addr = hub.local_addr().unwrap();

    let client_config = ClientConfig {
        bind_address: "127.0.0.1:0".parse().unwrap(),
        root_certificates: cert_chain,
        supported_compression_types: vec![CompressionType::None],
        keepalive_range: Duration::from_secs(1)..Duration::from_secs(10),
        protofish_config: Default::default(),
    };

    let tap = Arc::new(ZakofishTap::new(client_config).unwrap());
    let tap_handler = Arc::new(TestTapHandler);

    let tap_id = TapId(uuid::Uuid::new_v4());

    let hello_info = TapClientHello {
        tap_id: tap_id.clone(),
        friendly_name: "Test Tap".to_string(),
        api_token: "secret".to_string(),
        selection_weight: 1.0,
    };

    let tap_clone = tap.clone();
    tokio::spawn(async move {
        tap_clone
            .connect_and_run(local_addr, "localhost", hello_info, tap_handler)
            .await
            .unwrap();
    });

    tap_connected.notified().await;

    // Small delay to let Hub insert the tap into its sessions
    tokio::time::sleep(Duration::from_millis(50)).await;

    let ars = AudioRequestString::from("test:audio".to_string());
    let (success_msg, mut rx, _) = hub
        .request_audio(tap_id, ars, HashMap::new())
        .await
        .expect("Failed to request audio");

    assert_eq!(success_msg.duration_secs, Some(10.0));

    let mut received_chunks = 0;
    while let Some(chunks) = rx.recv().await {
        for chunk in chunks {
            assert_eq!(
                chunk.content,
                Bytes::from(format!("chunk {}", received_chunks))
            );
            received_chunks += 1;
        }
    }

    assert_eq!(received_chunks, 5);

    tracing::info!("Done!");
}
