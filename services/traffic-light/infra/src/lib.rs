use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use async_trait::async_trait;
use dashmap::DashMap;
use jsonrpsee::http_client::HttpClientBuilder;
use opentelemetry::global;
use tl_protocol::{AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineRpcClient};
use tokio::sync::RwLock;
use tracing::info;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use zako3_tl_core::{AeDispatcher, AeId, DiscordToken, SessionRoute, TlError, WorkerId, ZakoState};

use anyhow;

#[derive(Debug, Clone)]
pub enum RegistrationError {
    TokenNotFound,
    InvalidListenAddress(String),
    HttpClientBuild(String),
}

impl std::fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokenNotFound => write!(f, "Token not found in state"),
            Self::InvalidListenAddress(msg) => write!(f, "Invalid listen address: {}", msg),
            Self::HttpClientBuild(e) => write!(f, "Failed to build HTTP client: {}", e),
        }
    }
}

/// Validate a listen address format before registration.
/// Accepts formats like "127.0.0.1:8090" or "http://127.0.0.1:8090".
fn validate_listen_addr(addr: &str) -> Result<(), RegistrationError> {
    let trimmed = addr.trim();

    // Reject empty input
    if trimmed.is_empty() {
        return Err(RegistrationError::InvalidListenAddress(
            "Address cannot be empty".to_string(),
        ));
    }

    // Reject overly long input (RFC 1035 hostname limit is 253)
    if trimmed.len() > 300 {
        return Err(RegistrationError::InvalidListenAddress(
            "Address too long (max 300 chars)".to_string(),
        ));
    }

    // If no scheme, assume http:// will be prepended
    let url_str = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{}", trimmed)
    };

    // Try to parse as a URI to validate basic structure
    match url_str.parse::<http::Uri>() {
        Ok(uri) => {
            // Check for required components
            if uri.host().is_none() || uri.host().map(|h| h.is_empty()).unwrap_or(true) {
                return Err(RegistrationError::InvalidListenAddress(
                    "Host cannot be empty".to_string(),
                ));
            }

            // Check scheme is http or https
            let scheme_ok = uri
                .scheme()
                .map(|s| s == &http::uri::Scheme::HTTP || s == &http::uri::Scheme::HTTPS)
                .unwrap_or(false);
            if !scheme_ok {
                return Err(RegistrationError::InvalidListenAddress(
                    "Scheme must be http or https".to_string(),
                ));
            }

            Ok(())
        }
        Err(_) => Err(RegistrationError::InvalidListenAddress(
            "Invalid URL format".to_string(),
        )),
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
        // Validate address before consuming a token from the pool
        validate_listen_addr(&listen_addr)?;

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

    /// Heartbeat from an already-registered AE. Re-registers using the existing token
    /// without picking a new one from the pool.
    pub async fn heartbeat(&self, token: String, listen_addr: String) -> Result<(), RegistrationError> {
        validate_listen_addr(&listen_addr)?;
        self.register_with_token(DiscordToken(token), listen_addr).await
    }
}

#[async_trait]
impl AeDispatcher for AeRegistry {
    async fn send(
        &self,
        route: SessionRoute,
        mut request: AudioEngineCommandRequest,
    ) -> Result<AudioEngineCommandResponse, TlError> {
        let key = (route.worker_id, route.ae_id);
        let client_ref = self.clients.get(&key).ok_or(TlError::NoSuchAe)?;
        let client = client_ref.value();

        // Inject current span context into request headers for W3C trace propagation
        let cx = tracing::Span::current().context();
        global::get_text_map_propagator(|p| p.inject_context(&cx, &mut request.headers));

        // Call the RPC method
        let response = AudioEngineRpcClient::execute(client, request)
            .await
            .map_err(|e| TlError::Transport(e.to_string()))?;

        Ok(response)
    }
}
