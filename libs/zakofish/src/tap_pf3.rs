//! pf3 tap-side implementation. Uses the protofish3 chan/xfer API and the
//! [`crate::tap::TapHandler`] trait.

use protofish3::{
    ChanReceiver, ChanSender, Client, ClientConfig, ReconnectConfig, ReconnectingClient, XferMode,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::error::{Result, ZakofishError};
use crate::tap::TapHandler;
use crate::tap_streams::encode_pf3_chunk;
use crate::types::TransferMode;
use crate::types::message::{HubToTapMessage, TapClientHello, TapToHubMessage};

pub struct ZakofishTapPf3 {
    client: Arc<Client>,
}

impl ZakofishTapPf3 {
    pub fn new(client_config: ClientConfig) -> Result<Self> {
        let client = Client::bind(client_config)?;
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
            max_backoff: Duration::from_secs(8),
            backoff_multiplier: 2.0,
            max_retries: None,
        };

        let conn = ReconnectingClient::connect(
            self.client.clone(),
            hub_addr,
            server_name.to_string(),
            HashMap::new(),
            recon_config,
        )
        .await?;

        let mut reconnect_rx = conn.subscribe_reconnect();

        do_handshake(&conn, &hello_info).await?;

        loop {
            tokio::select! {
                _ = reconnect_rx.changed() => {
                    reconnect_rx.borrow_and_update();
                    tracing::info!("Reconnected to Hub, re-sending ClientHello");
                    if let Err(e) = do_handshake(&conn, &hello_info).await {
                        tracing::error!("Re-handshake failed: {:?}", e);
                    }
                }
                chan_result = conn.accept_chan() => {
                    match chan_result {
                        Ok((sender, receiver)) => {
                            let handler_clone = handler.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_incoming_chan(sender, receiver, handler_clone).await {
                                    tracing::error!("Error handling incoming chan: {:?}", e);
                                }
                            });
                        }
                        Err(e) => {
                            tracing::error!("Failed to accept chan: {:?}", e);
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

async fn do_handshake(conn: &ReconnectingClient, hello_info: &TapClientHello) -> Result<()> {
    let (sender, mut receiver) = conn.take_handshake_chan().await?;

    let hello_msg = TapToHubMessage::ClientHello(hello_info.clone());
    let encoded = crate::protocol::codec::encode_msgpack(&hello_msg)?;
    sender.send_msg(encoded.to_vec()).await?;

    let response_bytes = receiver.recv_msg().await?;
    let response: HubToTapMessage = crate::protocol::codec::decode_msgpack(&response_bytes)?;

    match response {
        HubToTapMessage::Accept => {
            tracing::info!("Tap connected and accepted by Hub.");
            Ok(())
        }
        HubToTapMessage::Reject(reject) => Err(ZakofishError::ProtocolError(format!(
            "Hub rejected connection: {:?}",
            reject
        ))),
        _ => Err(ZakofishError::ProtocolError(
            "Expected Accept or Reject".to_string(),
        )),
    }
}

async fn handle_incoming_chan(
    sender: ChanSender,
    mut receiver: ChanReceiver,
    handler: Arc<dyn TapHandler>,
) -> Result<()> {
    let payload_bytes = receiver.recv_msg().await?;
    let msg: HubToTapMessage = crate::protocol::codec::decode_msgpack(&payload_bytes)?;

    match msg {
        HubToTapMessage::AudioRequest(request) => {
            match handler
                .handle_audio_request(request.ars, request.headers)
                .await
            {
                Ok((success_msg, mut chunk_receiver, transfer_mode)) => {
                    let response_msg = TapToHubMessage::AudioRequestSuccess(success_msg);
                    sender
                        .send_msg(crate::protocol::codec::encode_msgpack(&response_msg)?.to_vec())
                        .await?;

                    let mut send_xfer = sender.start_xfer(map_mode(transfer_mode)).await?;

                    while let Some((timestamp, bytes)) = chunk_receiver.recv().await {
                        tracing::trace!(
                            "Sending pf3 chunk timestamp={} size={}",
                            timestamp.0,
                            bytes.len()
                        );
                        let buf = encode_pf3_chunk(timestamp, &bytes);
                        send_xfer.send(buf).await?;
                    }

                    send_xfer.end().await?;
                }
                Err(failure_msg) => {
                    let response_msg = TapToHubMessage::AudioRequestFailure(failure_msg);
                    sender
                        .send_msg(crate::protocol::codec::encode_msgpack(&response_msg)?.to_vec())
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
                    sender
                        .send_msg(crate::protocol::codec::encode_msgpack(&response_msg)?.to_vec())
                        .await?;
                }
                Err(failure_msg) => {
                    let response_msg = TapToHubMessage::AudioRequestFailure(failure_msg);
                    sender
                        .send_msg(crate::protocol::codec::encode_msgpack(&response_msg)?.to_vec())
                        .await?;
                }
            }
        }
        _ => {
            tracing::warn!("Received unexpected message on pf3 data chan: {:?}", msg);
        }
    }

    Ok(())
}

fn map_mode(mode: TransferMode) -> XferMode {
    match mode {
        TransferMode::Dual => XferMode::Dual,
        TransferMode::UnreliableOnly => XferMode::Unrel,
    }
}
