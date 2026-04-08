use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use zako3_types::AudioRequestString;
use zako3_types::hq::TapId;
use zakofish_taphub::create_server_config;
use zakofish_taphub::hub::{HubHandler, ZakofishHub};
use zakofish_taphub::types::message::{TapClientHello, TapServerReject};

fn generate_cert() -> (String, String) {
    let subject_alt_names = vec!["localhost".to_string()];
    let cert = rcgen::generate_simple_self_signed(subject_alt_names).unwrap();
    let pem_cert = cert.cert.pem();
    let pem_key = cert.signing_key.serialize_pem();
    (pem_cert, pem_key)
}

struct SimpleHubHandler {
    tap_connected_tx: mpsc::Sender<(TapId, u64)>,
}

#[async_trait::async_trait]
impl HubHandler for SimpleHubHandler {
    async fn on_tap_authenticate(
        &self,
        connection_id: u64,
        hello: TapClientHello,
    ) -> Result<(), TapServerReject> {
        println!(
            "Hub: Tap connected! ID: {:?}, Connection: {}",
            hello.tap_id, connection_id
        );
        let tap_id = TapId(hello.tap_id.0.clone());
        let _ = self.tap_connected_tx.send((tap_id, connection_id)).await;
        Ok(())
    }

    async fn on_tap_disconnected(&self, tap_id: TapId, connection_id: u64) {
        println!(
            "Hub: Tap disconnected! ID: {:?}, Connection: {}",
            tap_id, connection_id
        );
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Initialize rustls crypto provider
    let _ = rustls::crypto::ring::default_provider().install_default();

    let (pem_cert, pem_key) = generate_cert();

    // Save cert to file so tap can use it to trust us
    std::fs::write("hub.crt", pem_cert)?;
    std::fs::write("hub.key", pem_key)?;

    let server_config = create_server_config("127.0.0.1:4433".parse()?, "hub.crt", "hub.key")?;

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
    if let Some((tap_id, connection_id)) = tap_connected_rx.recv().await {
        println!(
            "Hub: Triggering a single transfer to Tap {:?} (conn: {})",
            tap_id, connection_id
        );

        // Give the hub a moment to finish registering the session in its internal map
        tokio::time::sleep(Duration::from_millis(100)).await;

        let ars: AudioRequestString = "test-audio-request".to_string().into();
        let headers = HashMap::new();

        // 1. Send the audio request
        match hub.request_audio(tap_id, connection_id, ars, headers).await {
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
