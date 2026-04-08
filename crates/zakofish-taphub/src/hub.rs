use protofish2::ManiTransferRecvStreams;
use protofish2::connection::{ProtofishServer, ServerConfig};
use protofish2::mani::transfer::recv::{TransferReliableRecvStream, TransferUnreliableRecvStream};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;
use zako3_types::AudioRequestString;
use zako3_types::hq::TapId;

use zakofish::error::{Result, ZakofishError};

type SessionMap = Arc<
    Mutex<
        HashMap<
            zakofish::types::TapId,
            HashMap<u64, Arc<Mutex<protofish2::connection::ProtofishConnection>>>,
        >,
    >,
>;

use zakofish::types::message::{
    AudioMetadataRequestMessage, AudioMetadataSuccessMessage, AudioRequestMessage,
    AudioRequestSuccessMessage, TapClientHello, TapServerReject,
};

#[async_trait::async_trait]
pub trait HubHandler: Send + Sync {
    async fn on_tap_authenticate(
        &self,
        connection_id: u64,
        hello: TapClientHello,
    ) -> std::result::Result<(), TapServerReject>;
    async fn on_tap_disconnected(&self, tap_id: TapId, connection_id: u64);
}

pub struct ZakofishHub {
    server: Arc<ProtofishServer>,
    handler: Arc<dyn HubHandler>,
    next_connection_id: Arc<AtomicU64>,
    sessions: SessionMap,
}

impl ZakofishHub {
    pub fn new(server_config: ServerConfig, handler: Arc<dyn HubHandler>) -> Result<Self> {
        let server = ProtofishServer::bind(server_config)?;
        Ok(Self {
            server: Arc::new(server),
            handler,
            next_connection_id: Arc::new(AtomicU64::new(1)),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.server.local_addr()
    }

    pub async fn run(&self) -> Result<()> {
        loop {
            let incoming = self.server.accept().await.ok_or_else(|| {
                ZakofishError::ProtocolError("Protofish server closed".to_string())
            })?;
            let conn = incoming.accept().await?;
            let handler = self.handler.clone();
            let sessions = self.sessions.clone();
            let next_connection_id = self.next_connection_id.clone();

            tokio::spawn(async move {
                let span = tracing::info_span!(
                    "tap.connection",
                    tap_id = tracing::field::Empty,
                    connection_id = tracing::field::Empty,
                    friendly_name = tracing::field::Empty,
                    disconnect_reason = tracing::field::Empty,
                );
                if let Err(e) =
                    handle_new_connection(conn, handler, sessions, next_connection_id, span).await
                {
                    tracing::error!("Error handling new connection: {:?}", e);
                }
            });
        }
    }

    pub async fn request_audio(
        &self,
        tap_id: TapId,
        connection_id: u64,
        ars: AudioRequestString,
        headers: HashMap<String, String>,
    ) -> Result<(
        AudioRequestSuccessMessage,
        TransferReliableRecvStream,
        TransferUnreliableRecvStream,
    )> {
        let wire_tap_id = zakofish::types::TapId(tap_id.0.clone());
        let wire_ars = zakofish::types::AudioRequestString(ars.to_string());

        let conn_arc = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(&wire_tap_id)
                .and_then(|m| m.get(&connection_id))
                .cloned()
                .ok_or_else(|| {
                    ZakofishError::ProtocolError(format!("Tap {} not connected", tap_id.0))
                })?
        };

        let mut conn = conn_arc.lock().await;
        let mut stream = conn.open_mani().await?;

        let request = AudioRequestMessage {
            ars: wire_ars,
            headers,
        };
        let payload = zakofish::types::message::HubToTapMessage::AudioRequest(request);
        let encoded = zakofish::protocol::codec::encode_msgpack(&payload)?;
        stream.send_payload(encoded).await?;

        let response_bytes = stream.recv_payload().await?;
        let response: zakofish::types::message::TapToHubMessage =
            zakofish::protocol::codec::decode_msgpack(&response_bytes)?;

        match response {
            zakofish::types::message::TapToHubMessage::AudioRequestSuccess(success) => {
                match stream.accept_transfer().await? {
                    ManiTransferRecvStreams::Dual {
                        reliable,
                        unreliable,
                    } => Ok((success, reliable, unreliable)),
                    _ => Err(ZakofishError::ProtocolError(
                        "Expected Dual transfer stream".to_string(),
                    )),
                }
            }
            zakofish::types::message::TapToHubMessage::AudioRequestFailure(failure) => Err(
                ZakofishError::ProtocolError(format!("Audio request failed: {:?}", failure)),
            ),
            _ => Err(ZakofishError::ProtocolError(
                "Unexpected response type".to_string(),
            )),
        }
    }

    pub async fn request_audio_metadata(
        &self,
        tap_id: TapId,
        connection_id: u64,
        ars: AudioRequestString,
        headers: HashMap<String, String>,
    ) -> Result<AudioMetadataSuccessMessage> {
        let wire_tap_id = zakofish::types::TapId(tap_id.0.clone());
        let wire_ars = zakofish::types::AudioRequestString(ars.to_string());

        let conn_arc = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(&wire_tap_id)
                .and_then(|m| m.get(&connection_id))
                .cloned()
                .ok_or_else(|| {
                    ZakofishError::ProtocolError(format!("Tap {} not connected", tap_id.0))
                })?
        };

        let mut conn = conn_arc.lock().await;
        let mut stream = conn.open_mani().await?;

        let request = AudioMetadataRequestMessage {
            ars: wire_ars,
            headers,
        };
        let payload =
            zakofish::types::message::HubToTapMessage::AudioMetadataRequest(request);
        let encoded = zakofish::protocol::codec::encode_msgpack(&payload)?;
        stream.send_payload(encoded).await?;

        let response_bytes = stream.recv_payload().await?;
        let response: zakofish::types::message::TapToHubMessage =
            zakofish::protocol::codec::decode_msgpack(&response_bytes)?;

        match response {
            zakofish::types::message::TapToHubMessage::AudioMetadataSuccess(success) => {
                Ok(success)
            }
            zakofish::types::message::TapToHubMessage::AudioRequestFailure(failure) => {
                Err(ZakofishError::ProtocolError(format!(
                    "Audio metadata request failed: {:?}",
                    failure
                )))
            }
            _ => Err(ZakofishError::ProtocolError(
                "Unexpected response type".to_string(),
            )),
        }
    }
}

async fn handle_new_connection(
    mut conn: protofish2::connection::ProtofishConnection,
    handler: Arc<dyn HubHandler>,
    sessions: SessionMap,
    next_connection_id: Arc<AtomicU64>,
    span: tracing::Span,
) -> Result<()> {
    let _enter = span.enter();

    let mut mani_stream = conn.accept_mani().await?;
    let payload_bytes = mani_stream.recv_payload().await?;

    let hello_msg: zakofish::types::message::TapToHubMessage =
        zakofish::protocol::codec::decode_msgpack(&payload_bytes)?;

    match hello_msg {
        zakofish::types::message::TapToHubMessage::ClientHello(hello) => {
            let tap_id_wire = hello.tap_id.clone();
            let connection_id = next_connection_id.fetch_add(1, Ordering::SeqCst);

            tracing::Span::current().record("tap_id", tracing::field::display(&tap_id_wire.0));
            tracing::Span::current().record("connection_id", connection_id);
            tracing::Span::current()
                .record("friendly_name", tracing::field::display(&hello.friendly_name));

            match handler.on_tap_authenticate(connection_id, hello).await {
                Ok(_) => {
                    let accept_msg = zakofish::types::message::HubToTapMessage::Accept;
                    mani_stream
                        .send_payload(
                            zakofish::protocol::codec::encode_msgpack(&accept_msg)?,
                        )
                        .await?;

                    let conn_arc = Arc::new(Mutex::new(conn));
                    sessions
                        .lock()
                        .await
                        .entry(tap_id_wire.clone())
                        .or_default()
                        .insert(connection_id, conn_arc.clone());

                    // Keep stream alive/wait for disconnect
                    let _ = mani_stream.recv_payload().await; // Wait until close or error

                    {
                        let mut sessions = sessions.lock().await;
                        if let Some(conns) = sessions.get_mut(&tap_id_wire) {
                            conns.remove(&connection_id);
                            if conns.is_empty() {
                                sessions.remove(&tap_id_wire);
                            }
                        }
                    }

                    tracing::Span::current().record("disconnect_reason", "clean");
                    // Convert wire TapId to zako3-types TapId for the public handler callback
                    let tap_id_public = TapId(tap_id_wire.0.clone());
                    handler.on_tap_disconnected(tap_id_public, connection_id).await;
                    Ok(())
                }
                Err(reject) => {
                    tracing::Span::current().record("disconnect_reason", "rejected");
                    let reject_msg = zakofish::types::message::HubToTapMessage::Reject(reject);
                    mani_stream
                        .send_payload(
                            zakofish::protocol::codec::encode_msgpack(&reject_msg)?,
                        )
                        .await?;
                    Ok(())
                }
            }
        }
        _ => Err(ZakofishError::ProtocolError(
            "Expected ClientHello".to_string(),
        )),
    }
}
