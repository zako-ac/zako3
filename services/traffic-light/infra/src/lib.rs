use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use dashmap::DashMap;
use tl_protocol::{AudioEngineCommandRequest, AudioEngineCommandResponse};
use tokio::sync::{Mutex, RwLock};
use tracing::{info, warn};
use zako3_ae_transport::TlServer;
use zako3_tl_core::{AeDispatcher, AeId, DiscordToken, SessionRoute, TlError, WorkerId, ZakoState};

pub struct AeRegistry {
    clients: DashMap<(WorkerId, AeId), Mutex<zako3_ae_transport::TlConnectedClient>>,
    state: Arc<RwLock<ZakoState>>,
    server: Mutex<TlServer>,
    token_pool: Vec<DiscordToken>,
    token_cursor: AtomicUsize,
}

impl AeRegistry {
    pub async fn new(
        addr: SocketAddr,
        state: Arc<RwLock<ZakoState>>,
        token_pool: Vec<DiscordToken>,
    ) -> Result<Self, zako3_ae_transport::TlError> {
        let server = TlServer::bind(addr).await?;
        Ok(Self {
            clients: DashMap::new(),
            state,
            server: Mutex::new(server),
            token_pool,
            token_cursor: AtomicUsize::new(0),
        })
    }

    pub fn state(&self) -> Arc<RwLock<ZakoState>> {
        self.state.clone()
    }

    pub async fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.server.lock().await.local_addr()
    }

    /// Accepts AE connections in a loop, provisioning each with a Discord token round-robin.
    pub async fn accept_loop(self: Arc<Self>) {
        loop {
            match self.accept_one().await {
                Ok((worker_id, ae_id)) => {
                    info!(worker_id = worker_id.0, ae_id = ae_id.0, "AE registered");

                    let mut state = self.state.write().await;
                    if let Some(worker) = state.workers.get_mut(&worker_id) {
                        if !worker.connected_ae_ids.contains(&ae_id.0) {
                            worker.connected_ae_ids.push(ae_id.0);
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Failed to accept AE connection, continuing");
                }
            }
        }
    }

    async fn accept_one(&self) -> Result<(WorkerId, AeId), zako3_ae_transport::TlError> {
        // Pick next token round-robin
        let idx = self.token_cursor.fetch_add(1, Ordering::Relaxed) % self.token_pool.len();
        let token = self.token_pool[idx].clone();

        // Accept connection and provision the AE with the assigned token
        let client = self
            .server
            .lock()
            .await
            .accept(
                zako3_ae_transport::DiscordToken(token.0.clone()),
                Default::default(),
            )
            .await?;

        let worker_id = {
            let state = self.state.read().await;
            state
                .workers
                .iter()
                .find(|(_, w)| w.discord_token == token)
                .map(|(id, _)| *id)
                .ok_or_else(|| {
                    zako3_ae_transport::TlError::Handshake("token not found in state".into())
                })?
        };

        // Evict stale entries for this worker to prevent state drift on reconnect
        let stale_ae_ids: Vec<AeId> = self
            .clients
            .iter()
            .filter(|e| e.key().0 == worker_id)
            .map(|e| e.key().1)
            .collect();

        for ae_id in &stale_ae_ids {
            self.clients.remove(&(worker_id, *ae_id));
        }

        // Assign new AE the starting ID (all old ones evicted, so reuse ID 1)
        let ae_id = AeId(1);

        // Remove stale ae_ids from worker's connected list
        if !stale_ae_ids.is_empty() {
            let mut state = self.state.write().await;
            if let Some(worker) = state.workers.get_mut(&worker_id) {
                worker.connected_ae_ids.retain(|id| !stale_ae_ids.iter().any(|ae| ae.0 == *id));
            }
        }

        self.clients.insert((worker_id, ae_id), Mutex::new(client));
        Ok((worker_id, ae_id))
    }
}

#[async_trait]
impl AeDispatcher for AeRegistry {
    async fn send(
        &self,
        route: SessionRoute,
        request: AudioEngineCommandRequest,
    ) -> Result<AudioEngineCommandResponse, TlError> {
        let key = (route.worker_id, route.ae_id);
        let entry = self.clients.get(&key).ok_or(TlError::NoSuchAe)?;
        entry
            .value()
            .lock()
            .await
            .request(request)
            .await
            .map_err(|e| TlError::Transport(e.to_string()))
    }
}
