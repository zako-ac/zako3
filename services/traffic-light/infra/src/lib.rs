use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use async_trait::async_trait;
use dashmap::DashMap;
use jsonrpsee::http_client::HttpClientBuilder;
use tl_protocol::{AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineRpcClient};
use tokio::sync::RwLock;
use tracing::info;
use zako3_tl_core::{AeDispatcher, AeId, DiscordToken, SessionRoute, TlError, WorkerId, ZakoState};

use anyhow;

#[derive(Debug, Clone)]
pub enum RegistrationError {
    TokenNotFound,
    HttpClientBuild(String),
}

impl std::fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokenNotFound => write!(f, "Token not found in state"),
            Self::HttpClientBuild(e) => write!(f, "Failed to build HTTP client: {}", e),
        }
    }
}

pub struct AeRegistry {
    // Map from (WorkerId, AeId) to the jsonrpsee HTTP client pointing to that AE's listen_addr
    clients: DashMap<(WorkerId, AeId), jsonrpsee::http_client::HttpClient>,
    state: Arc<RwLock<ZakoState>>,
    token_pool: Vec<DiscordToken>,
    token_cursor: AtomicUsize,
}

impl AeRegistry {
    pub async fn new(
        state: Arc<RwLock<ZakoState>>,
        token_pool: Vec<DiscordToken>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            clients: DashMap::new(),
            state,
            token_pool,
            token_cursor: AtomicUsize::new(0),
        })
    }

    pub fn state(&self) -> Arc<RwLock<ZakoState>> {
        self.state.clone()
    }

    /// Pick the next token from the pool using round-robin.
    fn pick_next_token(&self) -> DiscordToken {
        let idx = self.token_cursor.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.token_pool[idx % self.token_pool.len()].clone()
    }

    /// Register an AE that advertises itself at listen_addr. Picks the next token from the pool.
    /// Returns the assigned token. Evicts stale entries and updates state.
    pub async fn register(&self, listen_addr: String) -> Result<String, RegistrationError> {
        let token = self.pick_next_token();
        self.register_with_token(token.clone(), listen_addr)
            .await?;
        Ok(token.0)
    }

    /// Register an AE with a specific token. Internal method.
    async fn register_with_token(
        &self,
        token: DiscordToken,
        listen_addr: String,
    ) -> Result<(), RegistrationError> {
        // Find the WorkerId for this token
        let worker_id = {
            let state = self.state.read().await;
            state
                .workers
                .iter()
                .find(|(_, w)| w.discord_token == token)
                .map(|(id, _)| *id)
                .ok_or(RegistrationError::TokenNotFound)?
        };

        // Ensure the URL has a scheme; if it's just host:port, prepend http://
        let url = if listen_addr.starts_with("http://") || listen_addr.starts_with("https://") {
            listen_addr.clone()
        } else {
            format!("http://{}", listen_addr)
        };

        // Build the HTTP client to communicate with the AE
        let http_client = HttpClientBuilder::default()
            .build(&url)
            .map_err(|e| RegistrationError::HttpClientBuild(e.to_string()))?;

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

        // Assign the AE ID 1 (all stale ones are evicted, so we reuse ID 1)
        let ae_id = AeId(1);

        // Remove stale ae_ids from worker's connected list in state
        if !stale_ae_ids.is_empty() {
            let mut state = self.state.write().await;
            if let Some(worker) = state.workers.get_mut(&worker_id) {
                worker.connected_ae_ids.retain(|id| !stale_ae_ids.iter().any(|ae| ae.0 == *id));
            }
        }

        // Insert the new client
        self.clients.insert((worker_id, ae_id), http_client);

        // Add to worker's connected list
        {
            let mut state = self.state.write().await;
            if let Some(worker) = state.workers.get_mut(&worker_id) {
                if !worker.connected_ae_ids.contains(&ae_id.0) {
                    worker.connected_ae_ids.push(ae_id.0);
                }
            }
        }

        info!(
            worker_id = worker_id.0,
            ae_id = ae_id.0,
            listen_addr = %listen_addr,
            "AE registered"
        );

        Ok(())
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
        let client_ref = self.clients.get(&key).ok_or(TlError::NoSuchAe)?;
        let client = client_ref.value();

        // Inject tracing span into request headers
        // TODO: call inject_span here once it's moved to shared location

        // Call the RPC method
        let response = AudioEngineRpcClient::execute(client, request)
            .await
            .map_err(|e| TlError::Transport(e.to_string()))?;

        Ok(response)
    }
}
