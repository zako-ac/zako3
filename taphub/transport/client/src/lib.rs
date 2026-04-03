use protofish2::compression::CompressionType;
use protofish2::connection::{ClientConfig, ProtofishClient};
use protofish2::mani::transfer::jitter::OpusJitterBuffer;
use rustls::pki_types::CertificateDer;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

use zako3_taphub_transport_lib::{TapHubRequest, TapHubResponse};
use zako3_types::{AudioMetaResponse, AudioRequest, AudioResponse, CachedAudioRequest};

pub struct TransportClient {
    client: Arc<ProtofishClient>,
    conn: Arc<Mutex<Option<protofish2::connection::ProtofishConnection>>>,
    server_addr: SocketAddr,
    server_name: String,
}

impl TransportClient {
    pub fn new(
        bind_addr: SocketAddr,
        server_addr: SocketAddr,
        server_name: String,
        root_certificates: Vec<CertificateDer<'static>>,
    ) -> std::io::Result<Self> {
        let config = ClientConfig {
            bind_address: bind_addr,
            root_certificates,
            supported_compression_types: vec![CompressionType::None],
            keepalive_range: Duration::from_secs(1)..Duration::from_secs(10),
            protofish_config: Default::default(),
        };
        let client = ProtofishClient::bind(config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        Ok(Self {
            client: Arc::new(client),
            conn: Arc::new(Mutex::new(None)),
            server_addr,
            server_name,
        })
    }

    pub async fn connect(&self) -> Result<(), String> {
        let conn = self
            .client
            .connect(self.server_addr, &self.server_name, HashMap::new())
            .await
            .map_err(|e| e.to_string())?;

        let mut lock = self.conn.lock().await;
        *lock = Some(conn);
        Ok(())
    }

    async fn execute_request(&self, req: TapHubRequest) -> Result<TapHubResponse, String> {
        let mut lock = self.conn.lock().await;
        let conn = lock.as_mut().ok_or("Not connected".to_string())?;

        let mut stream = conn.open_mani().await.map_err(|e| e.to_string())?;

        let payload = rmp_serde::to_vec(&req).map_err(|e| e.to_string())?;
        stream
            .send_payload(payload.into())
            .await
            .map_err(|e| e.to_string())?;

        let resp_payload = stream.recv_payload().await.map_err(|e| e.to_string())?;
        let resp: TapHubResponse =
            rmp_serde::from_slice(&resp_payload).map_err(|e| e.to_string())?;

        // Return response, but wait, for AudioReady we need the stream to accept transfer
        // Let's modify logic to return the stream if it's AudioReady
        Ok(resp)
    }

    pub async fn request_audio(&self, req: CachedAudioRequest) -> Result<AudioResponse, String> {
        let mut lock = self.conn.lock().await;
        let conn = lock.as_mut().ok_or("Not connected".to_string())?;

        let mut stream = conn.open_mani().await.map_err(|e| e.to_string())?;

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
            _ => Err("Unexpected response".to_string()),
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
            _ => Err("Unexpected response".to_string()),
        }
    }

    pub async fn request_audio_meta(&self, req: AudioRequest) -> Result<AudioMetaResponse, String> {
        match self
            .execute_request(TapHubRequest::RequestAudioMeta(req))
            .await?
        {
            TapHubResponse::MetaReady(meta) => Ok(meta),
            TapHubResponse::Error(e) => Err(e),
            _ => Err("Unexpected response".to_string()),
        }
    }
}
