use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use protofish2::Timestamp;
use protofish2::compression::CompressionType;
use protofish2::connection::ClientConfig;
use rustls::pki_types::CertificateDer;
use uuid::Uuid;

use zakofish::tap::{TapHandler, ZakofishTap};
use zakofish::types::model::{AudioCachePolicy, AudioCacheType, AudioRequestString, TapId};
use zakofish::types::message::{
    AttachedMetadata, AudioMetadataSuccessMessage, AudioRequestFailureMessage,
    AudioRequestSuccessMessage, TapClientHello,
};

struct SimpleTapHandler;

#[async_trait::async_trait]
impl TapHandler for SimpleTapHandler {
    async fn handle_audio_request(
        &self,
        ars: AudioRequestString,
        _headers: HashMap<String, String>,
    ) -> Result<
        (
            AudioRequestSuccessMessage,
            mpsc::Receiver<(Timestamp, Bytes)>,
        ),
        AudioRequestFailureMessage,
    > {
        println!("Tap: Received audio request for: {}", ars);

        let (tx, rx) = mpsc::channel(10);

        // Spawn a task to generate some dummy audio data
        tokio::spawn(async move {
            for i in 0..5 {
                let dummy_data = Bytes::from(format!("dummy audio chunk {}", i));
                let ts = Timestamp(i as u64 * 100);
                if tx.send((ts, dummy_data)).await.is_err() {
                    break; // Hub disconnected
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            println!("Tap: Finished sending audio stream.");
        });

        Ok((
            AudioRequestSuccessMessage {
                cache: AudioCachePolicy {
                    cache_type: AudioCacheType::None,
                    ttl_seconds: None,
                },
                duration_secs: Some(1.0),
                metadatas: AttachedMetadata::Metadatas(vec![]),
            },
            rx,
        ))
    }

    async fn handle_audio_metadata_request(
        &self,
        ars: AudioRequestString,
        _headers: HashMap<String, String>,
    ) -> Result<AudioMetadataSuccessMessage, AudioRequestFailureMessage> {
        println!("Tap: Received audio metadata request for: {}", ars);
        Err(AudioRequestFailureMessage {
            reason: "Not implemented in example".to_string(),
            try_others: true,
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Initialize rustls crypto provider
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Read the hub's cert so we can trust it
    let cert_bytes =
        std::fs::read("hub.crt").expect("Please run the hub example first so it generates hub.crt");
    let cert_chain = vec![CertificateDer::from(cert_bytes)];

    let client_config = ClientConfig {
        bind_address: "127.0.0.1:0".parse()?,
        root_certificates: cert_chain,
        supported_compression_types: vec![CompressionType::None],
        keepalive_range: Duration::from_secs(1)..Duration::from_secs(10),
        protofish_config: Default::default(),
    };

    let tap = ZakofishTap::new(client_config)?;

    let hello_info = TapClientHello {
        tap_id: TapId(Uuid::new_v4().to_string()),
        friendly_name: "Simple Tap Example".to_string(),
        api_token: "secret_token".to_string(),
        selection_weight: 1.0,
    };

    println!("Tap: Connecting to Hub...");

    // Connect to the Hub running at 127.0.0.1:4433
    tap.connect_and_run(
        "127.0.0.1:4433".parse()?,
        "localhost", // server name
        hello_info,
        Arc::new(SimpleTapHandler),
    )
    .await?;

    Ok(())
}
