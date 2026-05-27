use protofish2::ManiTransferRecvStreams;
use protofish2::connection::{ProtofishServer, ServerConfig};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, oneshot};
use tracing::Instrument;
use zako3_types::AudioRequestString;
use zako3_types::hq::TapId;

use zakofish::error::{Result, ZakofishError};
use zakofish::tap_streams::{
    RelChunkStream, UnrelChunkStream, bridge_pf2_rel, bridge_pf2_unrel, bridge_pf3_recv,
};

#[derive(Clone)]
enum SessionConn {
    Pf2(Arc<protofish2::connection::ProtofishConnection>),
    Pf3(protofish3::Connection),
}

type SessionMap =
    Arc<Mutex<HashMap<zakofish::types::TapId, HashMap<u64, SessionConn>>>>;

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
    server_pf2: Option<Arc<ProtofishServer>>,
    server_pf3: Option<Arc<protofish3::Server>>,
    handler: Arc<dyn HubHandler>,
    next_connection_id: Arc<AtomicU64>,
    sessions: SessionMap,
}

impl ZakofishHub {
    pub fn new(
        server_config_pf2: Option<ServerConfig>,
        server_config_pf3: Option<protofish3::ServerConfig>,
        handler: Arc<dyn HubHandler>,
    ) -> Result<Self> {
        if server_config_pf2.is_none() && server_config_pf3.is_none() {
            return Err(ZakofishError::ProtocolError(
                "ZakofishHub requires at least one of pf2/pf3 server configs".to_string(),
            ));
        }

        let server_pf2 = server_config_pf2
            .map(ProtofishServer::bind)
            .transpose()?
            .map(Arc::new);
        let server_pf3 = server_config_pf3
            .map(protofish3::Server::bind)
            .transpose()?
            .map(Arc::new);

        Ok(Self {
            server_pf2,
            server_pf3,
            handler,
            next_connection_id: Arc::new(AtomicU64::new(1)),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn local_addr_pf2(&self) -> Option<std::io::Result<std::net::SocketAddr>> {
        self.server_pf2.as_ref().map(|s| s.local_addr())
    }

    pub fn local_addr_pf3(&self) -> Option<std::io::Result<std::net::SocketAddr>> {
        self.server_pf3.as_ref().map(|s| s.local_addr())
    }

    /// Backwards-compatible alias for the pf2 bind address. Returns the pf2
    /// `local_addr` if a pf2 server is configured, otherwise the pf3 address.
    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        if let Some(addr) = self.local_addr_pf2() {
            return addr;
        }
        if let Some(addr) = self.local_addr_pf3() {
            return addr;
        }
        Err(std::io::Error::other("no server bound"))
    }

    pub async fn run(&self) -> Result<()> {
        let mut set: tokio::task::JoinSet<Result<()>> = tokio::task::JoinSet::new();

        if let Some(server) = &self.server_pf2 {
            let server = server.clone();
            let handler = self.handler.clone();
            let sessions = self.sessions.clone();
            let next_connection_id = self.next_connection_id.clone();
            set.spawn(
                async move { run_pf2_loop(server, handler, sessions, next_connection_id).await },
            );
        }

        if let Some(server) = &self.server_pf3 {
            let server = server.clone();
            let handler = self.handler.clone();
            let sessions = self.sessions.clone();
            let next_connection_id = self.next_connection_id.clone();
            set.spawn(
                async move { run_pf3_loop(server, handler, sessions, next_connection_id).await },
            );
        }

        // Any loop returning is a fatal hub error — abort the rest and surface.
        let first = match set.join_next().await {
            Some(joined) => match joined {
                Ok(r) => r,
                Err(e) => Err(ZakofishError::ProtocolError(format!(
                    "transport loop join error: {e}"
                ))),
            },
            None => Ok(()),
        };
        set.abort_all();
        first
    }

    pub async fn request_audio(
        &self,
        tap_id: TapId,
        connection_id: u64,
        ars: AudioRequestString,
        headers: HashMap<String, String>,
    ) -> Result<(
        AudioRequestSuccessMessage,
        Option<RelChunkStream>,
        UnrelChunkStream,
    )> {
        let wire_tap_id = zakofish::types::TapId(tap_id.0.clone());
        let wire_ars = zakofish::types::AudioRequestString(ars.to_string());

        let session = self.get_session(&wire_tap_id, connection_id, &tap_id).await?;

        let request = AudioRequestMessage {
            ars: wire_ars,
            headers,
        };
        let payload = zakofish::types::message::HubToTapMessage::AudioRequest(request);
        let encoded = zakofish::protocol::codec::encode_msgpack(&payload)?;

        match session {
            SessionConn::Pf2(conn) => {
                let mut stream = conn.open_mani().await?;
                stream.send_payload(encoded).await?;
                let response_bytes = stream.recv_payload().await?;
                let response: zakofish::types::message::TapToHubMessage =
                    zakofish::protocol::codec::decode_msgpack(&response_bytes)?;

                match response {
                    zakofish::types::message::TapToHubMessage::AudioRequestSuccess(success) => {
                        match stream.accept_transfer().await? {
                            ManiTransferRecvStreams::Dual { reliable, unreliable } => Ok((
                                success,
                                Some(bridge_pf2_rel(reliable)),
                                bridge_pf2_unrel(unreliable),
                            )),
                            ManiTransferRecvStreams::UnreliableOnly { unreliable } => {
                                Ok((success, None, bridge_pf2_unrel(unreliable)))
                            }
                        }
                    }
                    zakofish::types::message::TapToHubMessage::AudioRequestFailure(failure) => {
                        Err(ZakofishError::TapRequestFailure {
                            reason: failure.reason,
                            try_others: failure.try_others,
                        })
                    }
                    _ => Err(ZakofishError::ProtocolError(
                        "Unexpected response type".to_string(),
                    )),
                }
            }
            SessionConn::Pf3(conn) => {
                let (sender, mut receiver) = conn.open_chan().await?;
                sender.send_msg(encoded.to_vec()).await?;
                let response_bytes = receiver.recv_msg().await?;
                let response: zakofish::types::message::TapToHubMessage =
                    zakofish::protocol::codec::decode_msgpack(&response_bytes)?;

                match response {
                    zakofish::types::message::TapToHubMessage::AudioRequestSuccess(success) => {
                        let (mode_tx, mode_rx) = oneshot::channel();
                        let (rel_cs, unrel_cs) = bridge_pf3_recv(receiver, mode_tx);
                        // sender is no longer needed by us; keep it alive until the
                        // bridge task finishes (so the chan isn't half-closed early).
                        // The bridge task owns the receiver; the sender drops here
                        // which is fine because the tap is purely downstream.
                        drop(sender);
                        let mode = mode_rx
                            .await
                            .map_err(|_| ZakofishError::ProtocolError(
                                "pf3 bridge dropped before signaling xfer mode".to_string(),
                            ))?;
                        let rel_out = match mode {
                            protofish3::XferMode::Dual | protofish3::XferMode::Rel => {
                                Some(rel_cs)
                            }
                            protofish3::XferMode::Unrel => None,
                        };
                        Ok((success, rel_out, unrel_cs))
                    }
                    zakofish::types::message::TapToHubMessage::AudioRequestFailure(failure) => {
                        Err(ZakofishError::TapRequestFailure {
                            reason: failure.reason,
                            try_others: failure.try_others,
                        })
                    }
                    _ => Err(ZakofishError::ProtocolError(
                        "Unexpected response type".to_string(),
                    )),
                }
            }
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

        let session = self.get_session(&wire_tap_id, connection_id, &tap_id).await?;

        let request = AudioMetadataRequestMessage {
            ars: wire_ars,
            headers,
        };
        let payload = zakofish::types::message::HubToTapMessage::AudioMetadataRequest(request);
        let encoded = zakofish::protocol::codec::encode_msgpack(&payload)?;

        match session {
            SessionConn::Pf2(conn) => {
                let mut stream = conn.open_mani().await?;
                stream.send_payload(encoded).await?;
                let response_bytes = stream.recv_payload().await?;
                let response: zakofish::types::message::TapToHubMessage =
                    zakofish::protocol::codec::decode_msgpack(&response_bytes)?;
                meta_dispatch(response)
            }
            SessionConn::Pf3(conn) => {
                let (sender, mut receiver) = conn.open_chan().await?;
                sender.send_msg(encoded.to_vec()).await?;
                let response_bytes = receiver.recv_msg().await?;
                let response: zakofish::types::message::TapToHubMessage =
                    zakofish::protocol::codec::decode_msgpack(&response_bytes)?;
                meta_dispatch(response)
            }
        }
    }

    async fn get_session(
        &self,
        wire_tap_id: &zakofish::types::TapId,
        connection_id: u64,
        tap_id: &TapId,
    ) -> Result<SessionConn> {
        let sessions = self.sessions.lock().await;
        sessions
            .get(wire_tap_id)
            .and_then(|m| m.get(&connection_id))
            .cloned()
            .ok_or_else(|| {
                ZakofishError::ProtocolError(format!("Tap {} not connected", tap_id.0))
            })
    }
}

fn meta_dispatch(
    response: zakofish::types::message::TapToHubMessage,
) -> Result<AudioMetadataSuccessMessage> {
    match response {
        zakofish::types::message::TapToHubMessage::AudioMetadataSuccess(success) => Ok(success),
        zakofish::types::message::TapToHubMessage::AudioRequestFailure(failure) => {
            Err(ZakofishError::TapRequestFailure {
                reason: failure.reason,
                try_others: failure.try_others,
            })
        }
        _ => Err(ZakofishError::ProtocolError(
            "Unexpected response type".to_string(),
        )),
    }
}

// Bounded wait for the application-level ClientHello after the protofish handshake
// completes. Without this, a client that opens a stream but never sends Hello
// keeps the spawned task, span, and connection alive until the QUIC idle
// timeout (which is much longer).
const CLIENT_HELLO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

async fn run_pf2_loop(
    server: Arc<ProtofishServer>,
    handler: Arc<dyn HubHandler>,
    sessions: SessionMap,
    next_connection_id: Arc<AtomicU64>,
) -> Result<()> {
    loop {
        let incoming = server.accept().await.ok_or_else(|| {
            ZakofishError::ProtocolError("Protofish2 server closed".to_string())
        })?;
        let handler = handler.clone();
        let sessions = sessions.clone();
        let next_connection_id = next_connection_id.clone();

        tokio::spawn(async move {
            let conn = match incoming.accept().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("pf2 QUIC handshake failed: {e}");
                    return;
                }
            };

            let ip = conn.quic_conn.remote_address().to_string();
            let span = tracing::info_span!(
                "tap.connection",
                transport = "pf2",
                tap_id = tracing::field::Empty,
                connection_id = tracing::field::Empty,
                friendly_name = tracing::field::Empty,
                disconnect_reason = tracing::field::Empty,
                remote_ip = %ip,
            );
            tracing::info!("New pf2 connection from {}", ip);

            if let Err(e) =
                handle_pf2_connection(conn, handler, sessions, next_connection_id)
                    .instrument(span)
                    .await
            {
                tracing::error!("Error handling pf2 connection: {:?}", e);
            }
        });
    }
}

async fn run_pf3_loop(
    server: Arc<protofish3::Server>,
    handler: Arc<dyn HubHandler>,
    sessions: SessionMap,
    next_connection_id: Arc<AtomicU64>,
) -> Result<()> {
    loop {
        let incoming = server.accept().await.ok_or_else(|| {
            ZakofishError::ProtocolError("Protofish3 server closed".to_string())
        })?;
        let handler = handler.clone();
        let sessions = sessions.clone();
        let next_connection_id = next_connection_id.clone();

        tokio::spawn(async move {
            let hs = match incoming.accept().await {
                Ok(h) => h,
                Err(e) => {
                    tracing::warn!("pf3 handshake failed: {e}");
                    return;
                }
            };

            let ip = hs.remote_addr().to_string();
            let span = tracing::info_span!(
                "tap.connection",
                transport = "pf3",
                tap_id = tracing::field::Empty,
                connection_id = tracing::field::Empty,
                friendly_name = tracing::field::Empty,
                disconnect_reason = tracing::field::Empty,
                remote_ip = %ip,
            );
            tracing::info!("New pf3 connection from {}", ip);

            if let Err(e) =
                handle_pf3_connection(hs, handler, sessions, next_connection_id)
                    .instrument(span)
                    .await
            {
                tracing::error!("Error handling pf3 connection: {:?}", e);
            }
        });
    }
}

async fn handle_pf2_connection(
    conn: protofish2::connection::ProtofishConnection,
    handler: Arc<dyn HubHandler>,
    sessions: SessionMap,
    next_connection_id: Arc<AtomicU64>,
) -> Result<()> {
    let mut mani_stream = conn.accept_mani().await?;
    let payload_bytes = tokio::time::timeout(CLIENT_HELLO_TIMEOUT, mani_stream.recv_payload())
        .await
        .map_err(|_| {
            ZakofishError::ProtocolError("Timed out waiting for ClientHello".to_string())
        })??;

    let hello_msg: zakofish::types::message::TapToHubMessage =
        zakofish::protocol::codec::decode_msgpack(&payload_bytes)?;

    let hello = match hello_msg {
        zakofish::types::message::TapToHubMessage::ClientHello(h) => h,
        _ => {
            return Err(ZakofishError::ProtocolError(
                "Expected ClientHello".to_string(),
            ));
        }
    };

    let tap_id_wire = hello.tap_id.clone();
    let connection_id = next_connection_id.fetch_add(1, Ordering::SeqCst);

    tracing::Span::current().record("tap_id", tracing::field::display(&tap_id_wire.0));
    tracing::Span::current().record("connection_id", connection_id);
    tracing::Span::current()
        .record("friendly_name", tracing::field::display(&hello.friendly_name));

    match handler.on_tap_authenticate(connection_id, hello).await {
        Ok(_) => {
            let tap_id_public = TapId(tap_id_wire.0.clone());
            let accept_msg = zakofish::types::message::HubToTapMessage::Accept;
            mani_stream
                .send_payload(zakofish::protocol::codec::encode_msgpack(&accept_msg)?)
                .await?;

            let conn_arc = Arc::new(conn);
            sessions
                .lock()
                .await
                .entry(tap_id_wire.clone())
                .or_default()
                .insert(connection_id, SessionConn::Pf2(conn_arc.clone()));

            drop(mani_stream);
            let _ = conn_arc.quic_conn.closed().await;

            let mut sessions = sessions.lock().await;
            if let Some(conns) = sessions.get_mut(&tap_id_wire) {
                conns.remove(&connection_id);
                if conns.is_empty() {
                    sessions.remove(&tap_id_wire);
                }
            }
            drop(sessions);

            handler.on_tap_disconnected(tap_id_public, connection_id).await;
            tracing::Span::current().record("disconnect_reason", "clean");
            Ok(())
        }
        Err(reject) => {
            tracing::Span::current().record("disconnect_reason", "rejected");
            let reject_msg = zakofish::types::message::HubToTapMessage::Reject(reject);
            mani_stream
                .send_payload(zakofish::protocol::codec::encode_msgpack(&reject_msg)?)
                .await?;
            Ok(())
        }
    }
}

async fn handle_pf3_connection(
    hs: protofish3::HandshakeRequest,
    handler: Arc<dyn HubHandler>,
    sessions: SessionMap,
    next_connection_id: Arc<AtomicU64>,
) -> Result<()> {
    let (conn, sender, mut receiver) = hs.accept(HashMap::new()).await?;

    let payload_bytes = tokio::time::timeout(CLIENT_HELLO_TIMEOUT, receiver.recv_msg())
        .await
        .map_err(|_| {
            ZakofishError::ProtocolError("Timed out waiting for ClientHello".to_string())
        })??;

    let hello_msg: zakofish::types::message::TapToHubMessage =
        zakofish::protocol::codec::decode_msgpack(&payload_bytes)?;

    let hello = match hello_msg {
        zakofish::types::message::TapToHubMessage::ClientHello(h) => h,
        _ => {
            return Err(ZakofishError::ProtocolError(
                "Expected ClientHello".to_string(),
            ));
        }
    };

    let tap_id_wire = hello.tap_id.clone();
    let connection_id = next_connection_id.fetch_add(1, Ordering::SeqCst);

    tracing::Span::current().record("tap_id", tracing::field::display(&tap_id_wire.0));
    tracing::Span::current().record("connection_id", connection_id);
    tracing::Span::current()
        .record("friendly_name", tracing::field::display(&hello.friendly_name));

    match handler.on_tap_authenticate(connection_id, hello).await {
        Ok(_) => {
            let tap_id_public = TapId(tap_id_wire.0.clone());
            let accept_msg = zakofish::types::message::HubToTapMessage::Accept;
            sender
                .send_msg(zakofish::protocol::codec::encode_msgpack(&accept_msg)?.to_vec())
                .await?;

            sessions
                .lock()
                .await
                .entry(tap_id_wire.clone())
                .or_default()
                .insert(connection_id, SessionConn::Pf3(conn.clone()));

            drop(sender);
            drop(receiver);
            conn.closed().await;

            let mut sessions = sessions.lock().await;
            if let Some(conns) = sessions.get_mut(&tap_id_wire) {
                conns.remove(&connection_id);
                if conns.is_empty() {
                    sessions.remove(&tap_id_wire);
                }
            }
            drop(sessions);

            handler.on_tap_disconnected(tap_id_public, connection_id).await;
            tracing::Span::current().record("disconnect_reason", "clean");
            Ok(())
        }
        Err(reject) => {
            tracing::Span::current().record("disconnect_reason", "rejected");
            let reject_msg = zakofish::types::message::HubToTapMessage::Reject(reject);
            sender
                .send_msg(zakofish::protocol::codec::encode_msgpack(&reject_msg)?.to_vec())
                .await?;
            Ok(())
        }
    }
}

