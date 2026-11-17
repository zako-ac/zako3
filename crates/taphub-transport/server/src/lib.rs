use std::collections::HashMap;

use protofish3::{ChanReceiver, ChanSender, Connection, Server, ServerConfig, XferMode};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::fs::File;
use std::future::Future;
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
        let conn_clone = conn.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_stream(sender, receiver, handler_clone, conn_clone).await {
                tracing::error!("Stream error: {:?}", e);
            }
        });
    }
}

/// Awaits `fut`, abandoning it the moment the peer's connection is gone.
///
/// Without this a client that has already given up on a request (the audio
/// engine resets its pooled connection when its own per-attempt timeout fires)
/// leaves the handler waiting out its whole budget, with a tap request in flight
/// that nobody is listening for. Dropping the handler future drops the request
/// chan with it, which is the whole of the cancellation here. The tap connection
/// itself is not touched: the handler only ever held a clone of it.
async fn until_client_gone<F, T>(conn: &Connection, fut: F) -> Option<T>
where
    F: Future<Output = T>,
{
    tokio::select! {
        biased;
        _ = conn.closed() => None,
        out = fut => Some(out),
    }
}

async fn handle_stream(
    sender: ChanSender,
    mut receiver: ChanReceiver,
    handler: Arc<dyn TapHubBridgeHandler>,
    conn: Connection,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let payload = receiver.recv_msg().await?;
    let req: TapHubRequest = rmp_serde::from_slice(&payload)?;

    match req {
        TapHubRequest::RequestAudio(req) => {
            let headers = req.headers.clone();
            let Some(result) =
                until_client_gone(&conn, handler.handle_request_audio(req, headers)).await
            else {
                tracing::warn!(
                    "client disconnected before its audio request was answered; dropping the request"
                );
                return Ok(());
            };
            match result {
                Ok((meta, mut chunk_receiver)) => {
                    let resp = TapHubResponse::AudioReady(meta);
                    sender.send_msg(rmp_serde::to_vec(&resp)?).await?;

                    let mut xfer = sender.start_xfer(XferMode::Unrel).await?;

                    loop {
                        tokio::select! {
                            biased;
                            _ = conn.closed() => {
                                // The receiver is gone; stop pulling frames out of the
                                // tap rather than draining a stream nobody reads. The
                                // dropped `SendXfer` asks the driver to end the transfer
                                // (and a busy driver may drop that request).
                                tracing::warn!(
                                    "client disconnected mid-stream; ending the transfer"
                                );
                                return Ok(());
                            }
                            chunk = chunk_receiver.recv() => match chunk {
                                Some((ts, bytes)) => xfer.send(encode_chunk(ts, &bytes)).await?,
                                None => break,
                            },
                        }
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
            let Some(result) =
                until_client_gone(&conn, handler.handle_preload_audio(req, headers)).await
            else {
                tracing::warn!("client disconnected before its preload request was answered");
                return Ok(());
            };
            let resp = match result {
                Ok(meta) => TapHubResponse::MetaReady(meta),
                Err(e) => TapHubResponse::Error(e),
            };
            sender.send_msg(rmp_serde::to_vec(&resp)?).await?;
        }
        TapHubRequest::RequestAudioMeta(req) => {
            let headers = req.headers.clone();
            let Some(result) =
                until_client_gone(&conn, handler.handle_request_audio_meta(req, headers)).await
            else {
                tracing::warn!("client disconnected before its metadata request was answered");
                return Ok(());
            };
            let resp = match result {
                Ok(meta) => TapHubResponse::MetaReady(meta),
                Err(e) => TapHubResponse::Error(e),
            };
            sender.send_msg(rmp_serde::to_vec(&resp)?).await?;
        }
        TapHubRequest::InvalidateCache(req) => {
            let headers = req.headers.clone();
            let Some(result) =
                until_client_gone(&conn, handler.handle_invalidate_cache(req, headers)).await
            else {
                tracing::warn!("client disconnected before its cache invalidation was answered");
                return Ok(());
            };
            let resp = match result {
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
