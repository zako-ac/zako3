use bytes::Bytes;
use protofish2::compression::CompressionType;
use protofish2::config::ReconnectConfig;
use protofish2::connection::{ClientConfig, ProtofishClient, ReconnectingConnection};
use protofish2::{Timestamp, TransferMode};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::error::{Result, ZakofishError};
use crate::types::message::{
    AudioMetadataSuccessMessage, AudioRequestFailureMessage, AudioRequestSuccessMessage,
    HubToTapMessage, TapClientHello, TapToHubMessage,
};
use crate::types::model::AudioRequestString;

#[async_trait::async_trait]
pub trait TapHandler: Send + Sync {
    /// Handle an incoming audio request.
    /// If successful, returns the success message and a receiver channel to push (Timestamp, Bytes) chunks into.
    /// If failed, returns the failure message.
    async fn handle_audio_request(
        &self,
        ars: AudioRequestString,
        headers: HashMap<String, String>,
    ) -> std::result::Result<
        (
            AudioRequestSuccessMessage,
            mpsc::Receiver<(Timestamp, Bytes)>,
        ),
        AudioRequestFailureMessage,
    >;

    /// Handle an incoming audio metadata request.
    /// If successful, returns the success message with metadata.
    /// If failed, returns the failure message.
    async fn handle_audio_metadata_request(
        &self,
        ars: AudioRequestString,
        headers: HashMap<String, String>,
    ) -> std::result::Result<AudioMetadataSuccessMessage, AudioRequestFailureMessage>;
}

pub struct ZakofishTap {
    client: Arc<ProtofishClient>,
}

impl ZakofishTap {
    pub fn new(client_config: ClientConfig) -> Result<Self> {
        let client = ProtofishClient::bind(client_config)?;
        Ok(Self {
            client: Arc::new(client),
        })
    }

    pub async fn connect_and_run(
        &self,
        hub_addr: std::net::SocketAddr,
        server_name: &str,
        hello_info: TapClientHello,
        handler: Arc<dyn TapHandler>,
    ) -> Result<()> {
        let recon_config = ReconnectConfig {
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(15),
            backoff_multiplier: 2.0,
            max_retries: None, // Keep trying indefinitely
        };

        let mut conn = ReconnectingConnection::connect(
            self.client.clone(),
            hub_addr,
            server_name.to_string(),
            HashMap::new(),
            recon_config,
        )
        .await?;

        // 1. Control Stream
        let mut control_stream = conn.open_mani().await?;

        let hello_msg = TapToHubMessage::ClientHello(hello_info);
        let encoded_hello = crate::protocol::codec::encode_msgpack(&hello_msg)?;
        control_stream.send_payload(encoded_hello).await?;

        let response_bytes = control_stream.recv_payload().await?;
        let response: HubToTapMessage = crate::protocol::codec::decode_msgpack(&response_bytes)?;

        match response {
            HubToTapMessage::Accept => {
                tracing::info!("Tap connected and accepted by Hub.");
            }
            HubToTapMessage::Reject(reject) => {
                return Err(ZakofishError::ProtocolError(format!(
                    "Hub rejected connection: {:?}",
                    reject
                )));
            }
            _ => {
                return Err(ZakofishError::ProtocolError(
                    "Expected Accept or Reject".to_string(),
                ));
            }
        }

        // Keep the control stream alive in a separate task, waiting for the server to close it
        tokio::spawn(async move {
            let _ = control_stream.recv_payload().await;
            tracing::warn!("Control stream closed by Hub.");
        });

        // 2. Listen for incoming AudioRequest streams
        loop {
            let mut mani_stream = match conn.accept_mani().await {
                Ok(stream) => stream,
                Err(e) => {
                    tracing::error!("Failed to accept mani stream: {:?}", e);
                    break;
                }
            };

            let handler_clone = handler.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_incoming_stream(&mut mani_stream, handler_clone).await {
                    tracing::error!("Error handling incoming stream: {:?}", e);
                }
            });
        }

        Ok(())
    }
}

async fn handle_incoming_stream(
    mani_stream: &mut protofish2::mani::stream::ManiStream,
    handler: Arc<dyn TapHandler>,
) -> Result<()> {
    let payload_bytes = mani_stream.recv_payload().await?;
    let msg: HubToTapMessage = crate::protocol::codec::decode_msgpack(&payload_bytes)?;

    match msg {
        HubToTapMessage::AudioRequest(request) => {
            match handler
                .handle_audio_request(request.ars, request.headers)
                .await
            {
                Ok((success_msg, mut chunk_receiver)) => {
                    // Send success payload
                    let response_msg = TapToHubMessage::AudioRequestSuccess(success_msg);
                    mani_stream
                        .send_payload(crate::protocol::codec::encode_msgpack(&response_msg)?)
                        .await?;

                    // Start transfer
                    let mut send_stream = mani_stream
                        .start_transfer(
                            TransferMode::Dual,
                            CompressionType::None,
                            protofish2::SequenceNumber(0),
                            None,
                        )
                        .await?;

                    // Stream chunks
                    while let Some((timestamp, bytes)) = chunk_receiver.recv().await {
                        tracing::debug!(
                            "Sending chunk with timestamp {:?} and size {}",
                            timestamp,
                            bytes.len()
                        );
                        send_stream.send(timestamp, bytes).await?;
                    }

                    // End transfer
                    send_stream.end().await?;
                }
                Err(failure_msg) => {
                    let response_msg = TapToHubMessage::AudioRequestFailure(failure_msg);
                    mani_stream
                        .send_payload(crate::protocol::codec::encode_msgpack(&response_msg)?)
                        .await?;
                }
            }
        }
        HubToTapMessage::AudioMetadataRequest(request) => {
            match handler
                .handle_audio_metadata_request(request.ars, request.headers)
                .await
            {
                Ok(success_msg) => {
                    let response_msg = TapToHubMessage::AudioMetadataSuccess(success_msg);
                    mani_stream
                        .send_payload(crate::protocol::codec::encode_msgpack(&response_msg)?)
                        .await?;
                }
                Err(failure_msg) => {
                    let response_msg = TapToHubMessage::AudioRequestFailure(failure_msg);
                    mani_stream
                        .send_payload(crate::protocol::codec::encode_msgpack(&response_msg)?)
                        .await?;
                }
            }
        }
        _ => {
            tracing::warn!("Received unexpected message on data stream: {:?}", msg);
        }
    }

    Ok(())
}
