use protofish2::compression::CompressionType;
use protofish2::connection::{ProtofishConnection, ProtofishServer, ServerConfig};
use protofish2::{Timestamp, TransferMode};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use zako3_taphub_transport_lib::{TapHubRequest, TapHubResponse};
use zako3_types::{AudioMetaResponse, AudioRequest, CachedAudioRequest};

#[async_trait::async_trait]
pub trait TapHubBridgeHandler: Send + Sync + 'static {
    async fn handle_request_audio(
        &self,
        req: CachedAudioRequest,
    ) -> Result<(AudioMetaResponse, mpsc::Receiver<(Timestamp, bytes::Bytes)>), String>;

    async fn handle_preload_audio(
        &self,
        req: CachedAudioRequest,
    ) -> Result<AudioMetaResponse, String>;

    async fn handle_request_audio_meta(
        &self,
        req: AudioRequest,
    ) -> Result<AudioMetaResponse, String>;
}

pub struct TransportServer {
    server: ProtofishServer,
    handler: Arc<dyn TapHubBridgeHandler>,
}

impl TransportServer {
    pub fn new(
        bind_addr: SocketAddr,
        cert_chain: Vec<CertificateDer<'static>>,
        private_key: PrivateKeyDer<'static>,
        handler: Arc<dyn TapHubBridgeHandler>,
    ) -> std::io::Result<Self> {
        let config = ServerConfig {
            bind_address: bind_addr,
            cert_chain,
            private_key,
            supported_compression_types: vec![CompressionType::None],
            keepalive_interval: Duration::from_secs(5),
            protofish_config: Default::default(),
        };
        let server =
            ProtofishServer::bind(config).map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(Self { server, handler })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.server
            .local_addr()
            .map_err(|e| std::io::Error::other(e.to_string()))
    }

    pub async fn run(&mut self) {
        loop {
            let incoming = match self.server.accept().await {
                Some(i) => i,
                None => break,
            };

            let conn = match incoming.accept().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Failed to accept connection: {:?}", e);
                    continue;
                }
            };

            let handler = self.handler.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(conn, handler).await {
                    tracing::error!("Connection error: {:?}", e);
                }
            });
        }
    }
}

async fn handle_connection(
    mut conn: ProtofishConnection,
    handler: Arc<dyn TapHubBridgeHandler>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let mut stream = conn.accept_mani().await?;
        let handler_clone = handler.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_stream(&mut stream, handler_clone).await {
                tracing::error!("Stream error: {:?}", e);
            }
        });
    }
}

async fn handle_stream(
    stream: &mut protofish2::mani::stream::ManiStream,
    handler: Arc<dyn TapHubBridgeHandler>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let payload = stream.recv_payload().await?;
    let req: TapHubRequest = rmp_serde::from_slice(&payload)?;

    match req {
        TapHubRequest::RequestAudio(req) => {
            match handler.handle_request_audio(req).await {
                Ok((meta, mut receiver)) => {
                    let resp = TapHubResponse::AudioReady(meta);
                    stream
                        .send_payload(rmp_serde::to_vec(&resp)?.into())
                        .await?;

                    // Start transfer using UnreliableOnly
                    let mut transfer = stream
                        .start_transfer(
                            TransferMode::UnreliableOnly,
                            CompressionType::None,
                            protofish2::SequenceNumber(0),
                            None,
                        )
                        .await?;

                    while let Some((ts, bytes)) = receiver.recv().await {
                        transfer.send(ts, bytes).await?;
                    }

                    transfer.end().await?;
                }
                Err(e) => {
                    let resp = TapHubResponse::Error(e);
                    stream
                        .send_payload(rmp_serde::to_vec(&resp)?.into())
                        .await?;
                }
            }
        }
        TapHubRequest::PreloadAudio(req) => {
            let resp = match handler.handle_preload_audio(req).await {
                Ok(meta) => TapHubResponse::MetaReady(meta),
                Err(e) => TapHubResponse::Error(e),
            };
            stream
                .send_payload(rmp_serde::to_vec(&resp)?.into())
                .await?;
        }
        TapHubRequest::RequestAudioMeta(req) => {
            let resp = match handler.handle_request_audio_meta(req).await {
                Ok(meta) => TapHubResponse::MetaReady(meta),
                Err(e) => TapHubResponse::Error(e),
            };
            stream
                .send_payload(rmp_serde::to_vec(&resp)?.into())
                .await?;
        }
    }

    Ok(())
}
