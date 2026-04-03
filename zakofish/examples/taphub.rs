use protofish2::compression::CompressionType;
use protofish2::connection::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use zako3_types::AudioRequestString;
use zako3_types::hq::TapId;
use zakofish::hub::{HubHandler, ZakofishHub};
use zakofish::types::message::{TapClientHello, TapServerReject};

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

struct SimpleHubHandler {
    tap_connected_tx: mpsc::Sender<TapId>,
}

#[async_trait::async_trait]
impl HubHandler for SimpleHubHandler {
    async fn on_tap_authenticate(&self, hello: TapClientHello) -> Result<(), TapServerReject> {
        println!("Hub: Tap connected! ID: {:?}", hello.tap_id);
        let _ = self.tap_connected_tx.send(hello.tap_id).await;
        Ok(())
    }

    async fn on_tap_disconnected(&self, tap_id: TapId) {
        println!("Hub: Tap disconnected! ID: {:?}", tap_id);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Initialize rustls crypto provider
    let _ = rustls::crypto::ring::default_provider().install_default();

    let (cert_chain, private_key) = generate_cert();

    // Save cert to file so tap can use it to trust us
    std::fs::write("hub.crt", &cert_chain[0])?;

    let server_config = ServerConfig {
        bind_address: "127.0.0.1:4433".parse()?,
        cert_chain,
        private_key,
        supported_compression_types: vec![CompressionType::None],
        keepalive_interval: Duration::from_secs(5),
        protofish_config: Default::default(),
    };

    let (tap_connected_tx, mut tap_connected_rx) = mpsc::channel(1);
    let handler = Arc::new(SimpleHubHandler { tap_connected_tx });

    // Instantiate the Hub
    let hub = Arc::new(ZakofishHub::new(server_config, handler)?);
    println!("Hub: Listening on {}", hub.local_addr()?);

    // Run the hub in the background
    let hub_clone = hub.clone();
    tokio::spawn(async move {
        if let Err(e) = hub_clone.run().await {
            println!("Hub run error: {:?}", e);
        }
    });

    // Wait for a Tap to connect
    println!("Hub: Waiting for a Tap to connect...");
    if let Some(tap_id) = tap_connected_rx.recv().await {
        println!("Hub: Triggering a single transfer to Tap {:?}", tap_id);

        // Give the hub a moment to finish registering the session in its internal map
        tokio::time::sleep(Duration::from_millis(100)).await;

        let ars: AudioRequestString = "test-audio-request".to_string().into();
        let headers = HashMap::new();

        // 1. Send the audio request
        match hub.request_audio(tap_id, ars, headers).await {
            Ok((success_msg, mut recv_stream, _)) => {
                println!(
                    "Hub: Received success response! Duration: {:?}s",
                    success_msg.duration_secs
                );

                // 2. Receive the audio data stream
                while let Some(chunks) = recv_stream.recv().await {
                    for chunk in chunks {
                        let ts = chunk.timestamp;
                        let payload = chunk.content;
                        println!(
                            "Hub: Received chunk from Tap! Timestamp: {:?}, Size: {} bytes, Data: {:?}",
                            ts,
                            payload.len(),
                            String::from_utf8_lossy(&payload)
                        );
                    }
                }

                println!("Hub: Stream ended.");
            }
            Err(e) => {
                println!("Hub: Audio request failed: {:?}", e);
            }
        }
    }

    // Give tap time to receive TransferEndAck before dropping the connection
    tokio::time::sleep(Duration::from_millis(500)).await;

    Ok(())
}
