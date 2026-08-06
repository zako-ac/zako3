mod jitter;

use protofish3::xfer::XferRecv;
use protofish3::{
    ChanReceiver, ChanSender, Client, ClientConfig, ReconnectConfig, ReconnectingClient, XferMode,
};
use rustls::pki_types::CertificateDer;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;

use jitter::OpusJitterBuffer;
use zako3_taphub_transport_lib::{TapHubRequest, TapHubResponse};
use zako3_types::{AudioMetaResponse, AudioRequest, AudioResponse, CachedAudioRequest, TapHubError};

const RESET_CLOSE_CODE: u64 = 1;
const MAX_ATTEMPTS: usize = 2;
/// Number of independent QUIC connections (all over one shared client endpoint) that
/// requests are round-robined across. A transport-level reset is scoped to the single
/// connection a failing request landed on, so it never tears down siblings on the other
/// pooled connections — the whole point of the pool.
const POOL_SIZE: usize = 4;

pub struct TransportClient {
    /// Pool of independent reconnecting connections. Requests round-robin across them via
    /// `cursor` so no single connection is a shared point of failure for the whole fleet.
    conns: Vec<Arc<ReconnectingClient>>,
    cursor: AtomicUsize,
    request_timeout: Duration,
}

impl TransportClient {
    pub async fn connect(
        bind_addr: SocketAddr,
        server_addr: &str,
        server_name: String,
        root_certificates: Vec<CertificateDer<'static>>,
        request_timeout: Duration,
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
        // Tight keepalive so stale (black-holed) connections are detected within
        // seconds instead of quinn's 45s default idle window.
        config.protofish.keepalive_interval = Some(Duration::from_secs(5));
        config.protofish.keepalive_timeout = Some(Duration::from_secs(15));

        // One shared QUIC client endpoint underlies every pooled connection; QUIC
        // multiplexes the independent connections over it.
        let client = Arc::new(Client::bind(config).map_err(|e| e.to_string())?);

        let reconnect_config = ReconnectConfig {
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            max_retries: None,
        };

        let mut conns = Vec::with_capacity(POOL_SIZE);
        for _ in 0..POOL_SIZE {
            let conn = ReconnectingClient::connect(
                Arc::clone(&client),
                server_addr,
                server_name.clone(),
                HashMap::new(),
                reconnect_config.clone(),
            )
            .await
            .map_err(|e| e.to_string())?;
            conns.push(Arc::new(conn));
        }

        Ok(Self {
            conns,
            cursor: AtomicUsize::new(0),
            request_timeout,
        })
    }

    /// Round-robin one connection out of the pool for a single request.
    fn pick_conn(&self) -> Arc<ReconnectingClient> {
        let idx = self.cursor.fetch_add(1, Ordering::Relaxed) % self.conns.len();
        Arc::clone(&self.conns[idx])
    }

    async fn round_trip_once(
        conn: &ReconnectingClient,
        payload: &[u8],
    ) -> protofish3::Result<(ChanSender, ChanReceiver, Vec<u8>)> {
        let (sender, mut receiver) = conn.open_chan().await?;
        sender.send_msg(payload.to_vec()).await?;
        let resp_payload = receiver.recv_msg().await?;
        Ok((sender, receiver, resp_payload))
    }

    /// One request/response exchange with a per-attempt timeout, isolated to a single
    /// pooled connection so a failure here never disturbs sibling requests on the other
    /// pooled connections. Recovery is scoped:
    /// - **Timeout**: the request's own QUIC stream is dropped when the future is
    ///   cancelled, and we reset *this* pooled connection before retrying so a wedged
    ///   connection (one that accepts the channel but never answers) is evicted from
    ///   the pool instead of being reused forever. Sibling connections are untouched.
    /// - **Transport error**: reset only *this* pooled connection, then retry.
    ///
    /// Application-level `TapHubResponse::Error` is returned as-is, never retried.
    ///
    /// Returns the pooled connection the exchange succeeded on so callers that keep
    /// streaming (e.g. `request_audio`) stay on the same connection.
    async fn round_trip(
        &self,
        payload: Vec<u8>,
    ) -> Result<(Arc<ReconnectingClient>, ChanSender, ChanReceiver, TapHubResponse), TapHubError>
    {
        let conn = self.pick_conn();
        for attempt in 1..=MAX_ATTEMPTS {
            match tokio::time::timeout(self.request_timeout, Self::round_trip_once(&conn, &payload))
                .await
            {
                Ok(Ok((sender, receiver, resp_payload))) => {
                    let resp: TapHubResponse = rmp_serde::from_slice(&resp_payload)
                        .map_err(|e| TapHubError::Internal(e.to_string()))?;
                    return Ok((conn, sender, receiver, resp));
                }
                Ok(Err(e)) if attempt < MAX_ATTEMPTS && is_transport_error(&e) => {
                    tracing::warn!(
                        error = %e,
                        attempt,
                        "taphub request failed on transport; resetting this pooled connection and retrying"
                    );
                    // Scoped to this one pooled connection — siblings are untouched.
                    conn.reset(RESET_CLOSE_CODE).await;
                }
                Ok(Err(e)) => return Err(TapHubError::Internal(e.to_string())),
                Err(_) if attempt < MAX_ATTEMPTS => {
                    tracing::warn!(
                        timeout_ms = self.request_timeout.as_millis() as u64,
                        attempt,
                        "taphub request timed out; resetting this pooled connection and retrying"
                    );
                    // A connection that accepts a channel but never answers is wedged;
                    // leaving it intact (as before) kept routing requests to a dead
                    // connection forever. Reset it so the retry lands on a fresh one.
                    conn.reset(RESET_CLOSE_CODE).await;
                }
                Err(_) => {
                    return Err(TapHubError::Internal(format!(
                        "taphub request timed out after {} attempts ({}ms each)",
                        MAX_ATTEMPTS,
                        self.request_timeout.as_millis()
                    )));
                }
            }
        }
        unreachable!("round_trip loop always returns")
    }

    async fn execute_request(&self, req: TapHubRequest) -> Result<TapHubResponse, TapHubError> {
        let payload = rmp_serde::to_vec(&req).map_err(|e| TapHubError::Internal(e.to_string()))?;
        let (_conn, _sender, _receiver, resp) = self.round_trip(payload).await?;
        Ok(resp)
    }

    pub async fn request_audio(&self, req: CachedAudioRequest) -> Result<AudioResponse, TapHubError> {
        let req_clone = req.clone();
        let payload = rmp_serde::to_vec(&TapHubRequest::RequestAudio(req))
            .map_err(|e| TapHubError::Internal(e.to_string()))?;
        let (conn, sender, mut receiver, resp) = self.round_trip(payload).await?;

        match resp {
            TapHubResponse::AudioReady(meta) => {
                let (tx, rx) = mpsc::channel(100);
                // Stream/invalidate on the same pooled connection the request landed on.
                let conn_clone = conn;
                let xfer_timeout = self.request_timeout;
                let invalidate_timeout = self.request_timeout;

                tokio::spawn(async move {
                    // Keep the sender alive for the duration of the transfer so the
                    // chan isn't half-closed before the unreliable xfer completes.
                    let _sender = sender;

                    let xfer =
                        match tokio::time::timeout(xfer_timeout, receiver.accept_xfer()).await {
                            Ok(Ok(x)) => x,
                            Ok(Err(e)) => {
                                tracing::error!("accept_xfer failed: {:?}", e);
                                return;
                            }
                            Err(_) => {
                                tracing::error!(
                                    timeout_ms = xfer_timeout.as_millis() as u64,
                                    "accept_xfer timed out"
                                );
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
                                    send_invalidate_cache(
                                        conn_clone,
                                        req_clone,
                                        invalidate_timeout,
                                    )
                                    .await;
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

fn is_transport_error(e: &protofish3::Error) -> bool {
    matches!(
        e,
        protofish3::Error::ConnectionClosed(_)
            | protofish3::Error::ChanClosed(_)
            | protofish3::Error::DriverGone
            | protofish3::Error::Quic(_)
            | protofish3::Error::Connect(_)
            | protofish3::Error::Write(_)
            | protofish3::Error::Read(_)
            | protofish3::Error::ReadExact(_)
            | protofish3::Error::Io(_)
            | protofish3::Error::HandshakeTimeout
    )
}

async fn send_invalidate_cache(
    conn: Arc<ReconnectingClient>,
    req: CachedAudioRequest,
    timeout: Duration,
) {
    let result = tokio::time::timeout(timeout, async move {
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
    })
    .await;

    if result.is_err() {
        tracing::warn!("send_invalidate_cache: timed out");
    }
}

pub fn load_certs<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
    Ok(certs)
}
