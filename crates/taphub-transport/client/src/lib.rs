mod jitter;

use protofish3::xfer::XferRecv;
use protofish3::{Client, ClientConfig, ReconnectConfig, ReconnectingClient, XferMode};
use rustls::pki_types::CertificateDer;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use jitter::OpusJitterBuffer;
use zako3_taphub_transport_lib::{TapHubRequest, TapHubResponse};
use zako3_types::{AudioMetaResponse, AudioRequest, AudioResponse, CachedAudioRequest, TapHubError};

pub struct TransportClient {
    conn: Arc<ReconnectingClient>,
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

        let mut config = ClientConfig::new(bind_addr);
        config.root_certificates = root_certificates;
        config.handshake_timeout = Duration::from_secs(10);
        config.protofish.xfer_credit_update_batch_size = Some(10);

        let client = Arc::new(Client::bind(config).map_err(|e| e.to_string())?);

        let reconnect_config = ReconnectConfig {
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            max_retries: None,
        };

        let conn = ReconnectingClient::connect(
            client,
            server_addr,
            server_name,
            HashMap::new(),
            reconnect_config,
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(Self {
            conn: Arc::new(conn),
        })
    }

    async fn execute_request(&self, req: TapHubRequest) -> Result<TapHubResponse, TapHubError> {
        let (sender, mut receiver) = self
            .conn
            .open_chan()
            .await
            .map_err(|e| TapHubError::Internal(e.to_string()))?;

        let payload = rmp_serde::to_vec(&req).map_err(|e| TapHubError::Internal(e.to_string()))?;
        sender
            .send_msg(payload)
            .await
            .map_err(|e| TapHubError::Internal(e.to_string()))?;

        let resp_payload = receiver
            .recv_msg()
            .await
            .map_err(|e| TapHubError::Internal(e.to_string()))?;
        let resp: TapHubResponse =
            rmp_serde::from_slice(&resp_payload).map_err(|e| TapHubError::Internal(e.to_string()))?;

        Ok(resp)
    }

    pub async fn request_audio(&self, req: CachedAudioRequest) -> Result<AudioResponse, TapHubError> {
        let (sender, mut receiver) = self
            .conn
            .open_chan()
            .await
            .map_err(|e| TapHubError::Internal(e.to_string()))?;

        let req_clone = req.clone();
        let payload = rmp_serde::to_vec(&TapHubRequest::RequestAudio(req))
            .map_err(|e| TapHubError::Internal(e.to_string()))?;
        sender
            .send_msg(payload)
            .await
            .map_err(|e| TapHubError::Internal(e.to_string()))?;

        let resp_payload = receiver
            .recv_msg()
            .await
            .map_err(|e| TapHubError::Internal(e.to_string()))?;
        let resp: TapHubResponse =
            rmp_serde::from_slice(&resp_payload).map_err(|e| TapHubError::Internal(e.to_string()))?;

        match resp {
            TapHubResponse::AudioReady(meta) => {
                let (tx, rx) = mpsc::channel(100);
                let conn_clone = Arc::clone(&self.conn);

                tokio::spawn(async move {
                    // Keep the sender alive for the duration of the transfer so the
                    // chan isn't half-closed before the unreliable xfer completes.
                    let _sender = sender;

                    let xfer = match receiver.accept_xfer().await {
                        Ok(x) => x,
                        Err(e) => {
                            tracing::error!("accept_xfer failed: {:?}", e);
                            return;
                        }
                    };

                    let single = match xfer {
                        XferRecv::Single(s) if s.mode() == XferMode::Unrel => s,
                        _ => {
                            tracing::error!("Expected Unrel single transfer");
                            return;
                        }
                    };

                    let mut jitter = match OpusJitterBuffer::new(
                        single,
                        48000,
                        opus::Channels::Stereo,
                        20,
                        100,
                    ) {
                        Ok(j) => j,
                        Err(e) => {
                            tracing::error!("Failed to create jitter buffer: {:?}", e);
                            return;
                        }
                    };

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
            _ => Err(TapHubError::Internal(format!(
                "Unexpected response to RequestAudio: {:?}",
                resp
            ))),
        }
    }

    pub async fn preload_audio(
        &self,
        req: CachedAudioRequest,
    ) -> Result<AudioMetaResponse, TapHubError> {
        match self
            .execute_request(TapHubRequest::PreloadAudio(req))
            .await?
        {
            TapHubResponse::MetaReady(meta) => Ok(meta),
            TapHubResponse::Error(e) => Err(e),
            resp => Err(TapHubError::Internal(format!(
                "Unexpected response to PreloadAudio: {:?}",
                resp
            ))),
        }
    }

    pub async fn request_audio_meta(
        &self,
        req: AudioRequest,
    ) -> Result<AudioMetaResponse, TapHubError> {
        match self
            .execute_request(TapHubRequest::RequestAudioMeta(req))
            .await?
        {
            TapHubResponse::MetaReady(meta) => Ok(meta),
            TapHubResponse::Error(e) => Err(e),
            resp => Err(TapHubError::Internal(format!(
                "Unexpected response to RequestAudioMeta: {:?}",
                resp
            ))),
        }
    }
}

async fn send_invalidate_cache(conn: Arc<ReconnectingClient>, req: CachedAudioRequest) {
    let (sender, mut receiver) = match conn.open_chan().await {
        Ok(c) => c,
        Err(_) => {
            tracing::warn!("send_invalidate_cache: failed to open chan");
            return;
        }
    };
    let Ok(payload) = rmp_serde::to_vec(&TapHubRequest::InvalidateCache(req)) else {
        tracing::warn!("send_invalidate_cache: failed to serialize request");
        return;
    };
    if sender.send_msg(payload).await.is_err() {
        tracing::warn!("send_invalidate_cache: failed to send payload");
        return;
    }
    if let Err(e) = receiver.recv_msg().await {
        tracing::warn!("send_invalidate_cache: failed to receive response: {:?}", e);
    }
}

pub fn load_certs<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
    Ok(certs)
}
