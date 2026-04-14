use protofish2::compression::CompressionType;
use protofish2::config::{ProtofishConfig, ReconnectConfig};
use protofish2::connection::{ClientConfig, ProtofishClient, ReconnectingConnection};
use protofish2::mani::transfer::jitter::OpusJitterBuffer;
use rustls::pki_types::CertificateDer;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

use zako3_taphub_transport_lib::{TapHubRequest, TapHubResponse};
use zako3_types::{AudioMetaResponse, AudioRequest, AudioResponse, CachedAudioRequest};

pub struct TransportClient {
    conn: Arc<Mutex<ReconnectingConnection>>,
}

impl TransportClient {
    pub async fn connect(
        bind_addr: SocketAddr,
        server_addr: &str,
        server_name: String,
        root_certificates: Vec<CertificateDer<'static>>,
    ) -> Result<Self, String> {
        let server_addr = server_addr
            .to_socket_addrs()
            .map_err(|e| format!("Failed to resolve '{}': {}", server_addr, e))?
            .next()
            .ok_or_else(|| format!("No addresses resolved for '{}'", server_addr))?;

        let mut protofish_config = ProtofishConfig::default();
        protofish_config.handshake_timeout = Duration::from_secs(10);

        let config = ClientConfig {
            bind_address: bind_addr,
            root_certificates,
            supported_compression_types: vec![CompressionType::None],
            keepalive_range: Duration::from_secs(1)..Duration::from_secs(30),
            protofish_config,
        };
        let client = Arc::new(ProtofishClient::bind(config).map_err(|e| e.to_string())?);

        let reconnect_config = ReconnectConfig {
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            max_retries: None,
        };

        let conn = ReconnectingConnection::connect(
            client,
            server_addr,
            server_name,
            HashMap::new(),
            reconnect_config,
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    async fn execute_request(&self, req: TapHubRequest) -> Result<TapHubResponse, String> {
        let mut stream = {
            let mut lock = self.conn.lock().await;
            lock.open_mani().await.map_err(|e| e.to_string())?
        };

        let payload = rmp_serde::to_vec(&req).map_err(|e| e.to_string())?;
        stream
            .send_payload(payload.into())
            .await
            .map_err(|e| e.to_string())?;

        let resp_payload = stream.recv_payload().await.map_err(|e| e.to_string())?;
        let resp: TapHubResponse =
            rmp_serde::from_slice(&resp_payload).map_err(|e| e.to_string())?;

        Ok(resp)
    }

    pub async fn request_audio(&self, req: CachedAudioRequest) -> Result<AudioResponse, String> {
        let mut stream = {
            let mut lock = self.conn.lock().await;
            lock.open_mani().await.map_err(|e| e.to_string())?
        };

        let req_clone = req.clone();
        let payload =
            rmp_serde::to_vec(&TapHubRequest::RequestAudio(req)).map_err(|e| e.to_string())?;
        stream
            .send_payload(payload.into())
            .await
            .map_err(|e| e.to_string())?;

        let resp_payload = stream.recv_payload().await.map_err(|e| e.to_string())?;
        let resp: TapHubResponse =
            rmp_serde::from_slice(&resp_payload).map_err(|e| e.to_string())?;

        match resp {
            TapHubResponse::AudioReady(meta) => {
                let transfer = stream.accept_transfer().await.map_err(|e| e.to_string())?;

                let unreliable_recv = match transfer {
                    protofish2::ManiTransferRecvStreams::UnreliableOnly { unreliable } => {
                        unreliable
                    }
                    _ => return Err("Expected UnreliableOnly transfer".to_string()),
                };

                let (tx, rx) = mpsc::channel(100);

                let mut jitter =
                    OpusJitterBuffer::new(unreliable_recv, 48000, opus::Channels::Stereo, 20, 100)
                        .map_err(|e| e.to_string())?;

                let conn_clone = Arc::clone(&self.conn);

                tokio::spawn(async move {
                    loop {
                        match jitter.yield_pcm().await {
                            Ok(Some(pcm)) => {
                                if tx.send(pcm).await.is_err() {
                                    break;
                                }
                            }
                            Ok(None) => break, // Stream ended
                            Err(e) => {
                                tracing::error!("Jitter buffer error: {:?}", e);
                                if e.to_string().contains("InvalidPacket") {
                                    tracing::warn!(
                                        "Invalid opus packet detected; invalidating cache"
                                    );
                                    send_invalidate_cache(conn_clone, req_clone).await;
                                }
                                break;
                            }
                        }
                    }
                });

                Ok(AudioResponse {
                    cache_key: Some(meta.cache_key),
                    metadatas: meta.metadatas,
                    stream: rx,
                })
            }
            TapHubResponse::Error(e) => Err(e),
            _ => Err(format!("Unexpected response to RequestAudio: {:?}", resp)),
        }
    }

    pub async fn preload_audio(
        &self,
        req: CachedAudioRequest,
    ) -> Result<AudioMetaResponse, String> {
        match self
            .execute_request(TapHubRequest::PreloadAudio(req))
            .await?
        {
            TapHubResponse::MetaReady(meta) => Ok(meta),
            TapHubResponse::Error(e) => Err(e),
            resp => Err(format!("Unexpected response to PreloadAudio: {:?}", resp)),
        }
    }

    pub async fn request_audio_meta(&self, req: AudioRequest) -> Result<AudioMetaResponse, String> {
        match self
            .execute_request(TapHubRequest::RequestAudioMeta(req))
            .await?
        {
            TapHubResponse::MetaReady(meta) => Ok(meta),
            TapHubResponse::Error(e) => Err(e),
            resp => Err(format!("Unexpected response to RequestAudioMeta: {:?}", resp)),
        }
    }
}

async fn send_invalidate_cache(conn: Arc<Mutex<ReconnectingConnection>>, req: CachedAudioRequest) {
    let mut stream = {
        let mut lock = conn.lock().await;
        match lock.open_mani().await {
            Ok(s) => s,
            Err(_) => {
                tracing::warn!("send_invalidate_cache: failed to open stream");
                return;
            }
        }
    };
    let Ok(payload) = rmp_serde::to_vec(&TapHubRequest::InvalidateCache(req)) else {
        tracing::warn!("send_invalidate_cache: failed to serialize request");
        return;
    };
    if stream.send_payload(payload.into()).await.is_err() {
        tracing::warn!("send_invalidate_cache: failed to send payload");
        return;
    }
    if let Err(e) = stream.recv_payload().await {
        tracing::warn!("send_invalidate_cache: failed to receive response: {:?}", e);
    }
}

pub fn load_certs<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
    Ok(certs)
}
