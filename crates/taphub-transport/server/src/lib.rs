use std::collections::HashMap;

use protofish3::{ChanReceiver, ChanSender, Connection, Server, ServerConfig, XferMode};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use zako3_taphub_transport_lib::{TapHubRequest, TapHubResponse, encode_chunk};
pub use zako3_taphub_transport_lib::Timestamp;
use zako3_types::{AudioMetaResponse, AudioRequest, CachedAudioRequest, TapHubError};

#[async_trait::async_trait]
pub trait TapHubBridgeHandler: Send + Sync + 'static {
    async fn handle_request_audio(
        &self,
        req: CachedAudioRequest,
        headers: HashMap<String, String>,
    ) -> Result<(AudioMetaResponse, mpsc::Receiver<(Timestamp, bytes::Bytes)>), TapHubError>;

    async fn handle_preload_audio(
        &self,
        req: CachedAudioRequest,
        headers: HashMap<String, String>,
    ) -> Result<AudioMetaResponse, TapHubError>;

    async fn handle_request_audio_meta(
        &self,
        req: AudioRequest,
        headers: HashMap<String, String>,
    ) -> Result<AudioMetaResponse, TapHubError>;

    async fn handle_invalidate_cache(
        &self,
        req: CachedAudioRequest,
        headers: HashMap<String, String>,
    ) -> Result<(), TapHubError>;
}

pub struct TransportServer {
    server: Server,
    handler: Arc<dyn TapHubBridgeHandler>,
}

impl TransportServer {
    pub fn new(
        bind_addr: SocketAddr,
        cert_chain: Vec<CertificateDer<'static>>,
        private_key: PrivateKeyDer<'static>,
        handler: Arc<dyn TapHubBridgeHandler>,
    ) -> std::io::Result<Self> {
        let mut config = ServerConfig::new(bind_addr, cert_chain, private_key);
        config.protofish.keepalive_interval = Some(Duration::from_secs(5));

        if let Ok(var) = std::env::var("PF_INITIAL_BACKPRESSURE_CREDITS") {
            if let Ok(credits) = var.parse::<u32>() {
                tracing::info!("Setting initial backpressure credits to {}", credits);
                config.protofish.initial_backpressure_credits = credits;
            }
        }

        let server = Server::bind(config).map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(Self { server, handler })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.server.local_addr()
    }

    pub async fn run(&mut self) {
        loop {
            let incoming = match self.server.accept().await {
                Some(i) => i,
                None => break,
            };

            let handler = self.handler.clone();
            tokio::spawn(async move {
                let hs = match incoming.accept().await {
                    Ok(h) => h,
                    Err(e) => {
                        tracing::error!("Failed to accept handshake: {:?}", e);
                        return;
                    }
                };

                // taphub-transport has no app-level hello; accept the connection and
                // discard the handshake chan halves.
                let (conn, _hs_sender, _hs_receiver) = match hs.accept(HashMap::new()).await {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("Failed to accept connection: {:?}", e);
                        return;
                    }
                };

                if let Err(e) = handle_connection(conn, handler).await {
                    tracing::error!("Connection error: {:?}", e);
                }
            });
        }
    }
}

async fn handle_connection(
    conn: Connection,
    handler: Arc<dyn TapHubBridgeHandler>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let (sender, receiver) = conn.accept_chan().await?;
        let handler_clone = handler.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_stream(sender, receiver, handler_clone).await {
                tracing::error!("Stream error: {:?}", e);
            }
        });
    }
}

async fn handle_stream(
    sender: ChanSender,
    mut receiver: ChanReceiver,
    handler: Arc<dyn TapHubBridgeHandler>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let payload = receiver.recv_msg().await?;
    let req: TapHubRequest = rmp_serde::from_slice(&payload)?;

    match req {
        TapHubRequest::RequestAudio(req) => {
            let headers = req.headers.clone();
            match handler.handle_request_audio(req, headers).await {
                Ok((meta, mut chunk_receiver)) => {
                    let resp = TapHubResponse::AudioReady(meta);
                    sender.send_msg(rmp_serde::to_vec(&resp)?).await?;

                    let mut xfer = sender.start_xfer(XferMode::Unrel).await?;

                    while let Some((ts, bytes)) = chunk_receiver.recv().await {
                        xfer.send(encode_chunk(ts, &bytes)).await?;
                    }

                    xfer.end().await?;
                }
                Err(e) => {
                    let resp = TapHubResponse::Error(e);
                    sender.send_msg(rmp_serde::to_vec(&resp)?).await?;
                }
            }
        }
        TapHubRequest::PreloadAudio(req) => {
            let headers = req.headers.clone();
            let resp = match handler.handle_preload_audio(req, headers).await {
                Ok(meta) => TapHubResponse::MetaReady(meta),
                Err(e) => TapHubResponse::Error(e),
            };
            sender.send_msg(rmp_serde::to_vec(&resp)?).await?;
        }
        TapHubRequest::RequestAudioMeta(req) => {
            let headers = req.headers.clone();
            let resp = match handler.handle_request_audio_meta(req, headers).await {
                Ok(meta) => TapHubResponse::MetaReady(meta),
                Err(e) => TapHubResponse::Error(e),
            };
            sender.send_msg(rmp_serde::to_vec(&resp)?).await?;
        }
        TapHubRequest::InvalidateCache(req) => {
            let headers = req.headers.clone();
            let resp = match handler.handle_invalidate_cache(req, headers).await {
                Ok(()) => TapHubResponse::InvalidateCacheOk,
                Err(e) => TapHubResponse::Error(e),
            };
            sender.send_msg(rmp_serde::to_vec(&resp)?).await?;
        }
    }

    Ok(())
}

pub fn load_certs_and_key<P: AsRef<Path>>(
    cert_path: P,
    key_path: P,
) -> std::io::Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    let cert_file = File::open(cert_path)?;
    let mut cert_reader = BufReader::new(cert_file);
    let cert_chain = rustls_pemfile::certs(&mut cert_reader).collect::<Result<Vec<_>, _>>()?;

    let key_file = File::open(key_path)?;
    let mut key_reader = BufReader::new(key_file);
    let private_key = rustls_pemfile::private_key(&mut key_reader)?.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "No private key found in file",
        )
    })?;

    Ok((cert_chain, private_key))
}
