use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use async_trait::async_trait;
use dashmap::DashMap;
use jsonrpsee::http_client::HttpClientBuilder;
use opentelemetry::global;
use tl_protocol::{AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineRpcClient};
use tokio::sync::RwLock;
use tracing::{debug, info};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use zako3_tl_core::{AeDispatcher, AeId, DiscordToken, SessionRoute, TlError, WorkerId, ZakoState};

use anyhow;

#[derive(Debug, Clone)]
pub enum RegistrationError {
    TokenNotFound,
    InvalidListenAddress(String),
    HttpClientBuild(String),
    OrdinalOutOfRange { ordinal: usize, pool: usize },
}

impl std::fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokenNotFound => write!(f, "Token not found in state"),
            Self::InvalidListenAddress(msg) => write!(f, "Invalid listen address: {}", msg),
            Self::HttpClientBuild(e) => write!(f, "Failed to build HTTP client: {}", e),
            Self::OrdinalOutOfRange { ordinal, pool } => write!(
                f,
                "AE pod ordinal {} has no matching token (pool size {}); \
                 DISCORD_TOKENS must have at least as many tokens as AE replicas",
                ordinal, pool
            ),
        }
    }
}

fn normalize_url(addr: &str) -> String {
    if addr.starts_with("http://") || addr.starts_with("https://") {
        addr.to_string()
    } else {
        format!("http://{}", addr)
    }
}

/// Extract the StatefulSet pod ordinal from an AE's advertised address.
///
/// AEs advertise their pod hostname, e.g.
/// `http://zako3-audio-engine-3.zako3-audio-engine.<ns>.svc.cluster.local:8090` → `3`.
/// That ordinal is a permanent, unique-per-pod identity, so using it as the worker index makes
/// the AE→worker mapping a pure function of pod identity — collision-free by construction and
/// stable across any restart, independent of registration order or timing.
///
/// Returns `None` for addresses with no trailing ordinal (e.g. `http://127.0.0.1:8090` in local
/// dev or tests), signalling the caller to fall back to dynamic assignment.
fn worker_ordinal(addr: &str) -> Option<usize> {
    let host = addr
        .strip_prefix("http://")
        .or_else(|| addr.strip_prefix("https://"))
        .unwrap_or(addr);
    // First DNS label is the pod hostname; the segment after its final '-' is the ordinal.
    let label = host.split(['.', ':', '/']).next()?;
    label.rsplit_once('-')?.1.parse::<usize>().ok()
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
    // Stable mapping from normalized listen_addr URL to WorkerId, so re-registrations from the
    // same address always land on the same worker instead of drifting via round-robin.
    addr_to_worker: DashMap<String, WorkerId>,
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
            addr_to_worker: DashMap::new(),
            state,
            token_pool,
            token_cursor: AtomicUsize::new(0),
        })
    }

    pub fn state(&self) -> Arc<RwLock<ZakoState>> {
        self.state.clone()
    }

    /// Pick a token whose worker currently has no connected AE ("free"), scanning the pool in
    /// round-robin order from the cursor. Falls back to plain round-robin only when every worker
    /// is already serving an AE (pool oversubscribed), so registration degrades gracefully
    /// instead of silently handing out a token that is already live on another engine.
    ///
    /// Plain round-robin (`cursor % len`) is unsafe here: once the cursor passes the pool size —
    /// which any AE restart from a new listen_addr causes — it wraps onto a token still held by a
    /// live engine, so two AEs log in as the same bot and Discord drops one of them.
    async fn pick_free_token(&self) -> DiscordToken {
        let n = self.token_pool.len();
        let start = self
            .token_cursor
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // Tokens whose worker already has a live AE attached.
        let in_use: Vec<DiscordToken> = {
            let state = self.state.read().await;
            state
                .workers
                .values()
                .filter(|w| !w.connected_ae_ids.is_empty())
                .map(|w| w.discord_token.clone())
                .collect()
        };

        for offset in 0..n {
            let token = &self.token_pool[(start + offset) % n];
            if !in_use.iter().any(|t| t == token) {
                return token.clone();
            }
        }

        self.token_pool[start % n].clone()
    }

    /// Register an AE that advertises itself at listen_addr. Returns the assigned token.
    /// Evicts stale entries and updates state.
    ///
    /// Worker assignment is derived from the pod's StatefulSet ordinal (parsed from the advertised
    /// hostname): pod `-N` always maps to `WorkerId(N)` / `token_pool[N]`. Because this is a pure
    /// function of pod identity, it is collision-free and identical across every restart — no two
    /// AEs can ever be handed the same bot token, and no worker slot is skipped. This replaces the
    /// earlier round-robin/sticky-map scheme, whose order-dependence let two pods collide on one
    /// token (leaving a worker with no AE) in a way no restart could clear.
    ///
    /// Addresses without an ordinal (local dev / tests, e.g. `127.0.0.1:PORT`) fall back to the
    /// address-sticky + free-token assignment, which is safe there since a single AE runs.
    pub async fn register(&self, listen_addr: String) -> Result<String, RegistrationError> {
        // Validate address before consuming a token from the pool
        validate_listen_addr(&listen_addr)?;

        let url = normalize_url(&listen_addr);

        let token = if let Some(ordinal) = worker_ordinal(&url) {
            // Production: pod ordinal is the authoritative worker index.
            self.token_pool
                .get(ordinal)
                .cloned()
                .ok_or(RegistrationError::OrdinalOutOfRange {
                    ordinal,
                    pool: self.token_pool.len(),
                })?
        } else if let Some(worker_id) = self.addr_to_worker.get(&url).map(|e| *e) {
            // Dev fallback: keep the address sticky so a restart doesn't drift to a new worker.
            let state = self.state.read().await;
            state
                .workers
                .get(&worker_id)
                .map(|w| w.discord_token.clone())
                .ok_or(RegistrationError::TokenNotFound)?
        } else {
            self.pick_free_token().await
        };

        self.register_with_token(token.clone(), listen_addr, true).await?;
        Ok(token.0)
    }

    /// Register an AE with a specific token. Internal method.
    /// `evict_sessions` should be `true` only for fresh registrations (AE restart) so that
    /// stale TL-cached sessions are cleared. Heartbeats must pass `false` — they refresh the
    /// client connection without invalidating active sessions.
    async fn register_with_token(
        &self,
        token: DiscordToken,
        listen_addr: String,
        evict_sessions: bool,
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

        let url = normalize_url(&listen_addr);

        // Heartbeat fast-path: if this AE already has a live client registered for the same
        // address, reuse it instead of tearing down and rebuilding the HTTP client every
        // 15s cycle. The rebuild (remove -> await state lock -> reinsert) opened a brief
        // window where a concurrent dispatch saw NoSuchAe / a fresh-connect error, which
        // sync_sessions/reconcile could misread as a dead session and kick the bot.
        if !evict_sessions
            && self.clients.contains_key(&(worker_id, AeId(1)))
            && self.addr_to_worker.get(&url).map(|e| *e) == Some(worker_id)
        {
            // Defensive: make sure the worker still advertises this ae_id.
            {
                let mut state = self.state.write().await;
                if let Some(worker) = state.workers.get_mut(&worker_id) {
                    if !worker.connected_ae_ids.contains(&1) {
                        worker.connected_ae_ids.push(1);
                    }
                }
            }
            debug!(
                worker_id = worker_id.0,
                ae_id = 1,
                listen_addr = %listen_addr,
                "AE heartbeat"
            );
            return Ok(());
        }

        // Build the HTTP client to communicate with the AE
        let http_client = HttpClientBuilder::default()
            .request_timeout(std::time::Duration::from_secs(15))
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

        // Remove stale ae_ids from state, and sessions if this is a fresh registration
        if !stale_ae_ids.is_empty() {
            let mut state = self.state.write().await;
            if let Some(worker) = state.workers.get_mut(&worker_id) {
                worker
                    .connected_ae_ids
                    .retain(|id| !stale_ae_ids.iter().any(|ae| ae.0 == *id));
            }
            // Only evict sessions on fresh `register()` calls, not heartbeats. Heartbeats refresh
            // the client connection but the AE still owns its sessions; evicting here would cause
            // reconcile to see discord=1/cache=0 and kick the bot every heartbeat cycle.
            if evict_sessions {
                for stale_ae_id in &stale_ae_ids {
                    state
                        .sessions
                        .retain(|route, _| !(route.worker_id == worker_id && route.ae_id == *stale_ae_id));
                }
            }
        }

        // Record the stable addr→worker mapping for future re-registrations from this address.
        self.addr_to_worker.insert(url, worker_id);

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

        if evict_sessions {
            // Fresh register() — a real (re)start of this AE.
            info!(
                worker_id = worker_id.0,
                ae_id = ae_id.0,
                listen_addr = %listen_addr,
                "AE registered"
            );
        } else {
            // Heartbeat that had to rebuild the client (TL had no live client for it).
            // Noteworthy because it means the previous client was missing.
            info!(
                worker_id = worker_id.0,
                ae_id = ae_id.0,
                listen_addr = %listen_addr,
                "AE client re-established (heartbeat)"
            );
        }

        Ok(())
    }

    /// Heartbeat from an already-registered AE. Re-registers using the existing token
    /// without picking a new one from the pool.
    pub async fn heartbeat(
        &self,
        token: String,
        listen_addr: String,
    ) -> Result<(), RegistrationError> {
        validate_listen_addr(&listen_addr)?;
        self.register_with_token(DiscordToken(token), listen_addr, false)
            .await
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
        // fuck the DashMap lock
        let client = self.clients.get(&key).ok_or(TlError::NoSuchAe)?.clone();

        // Inject current span context into request headers for W3C trace propagation
        let cx = tracing::Span::current().context();
        global::get_text_map_propagator(|p| p.inject_context(&cx, &mut request.headers));

        // Call the RPC method
        let response = AudioEngineRpcClient::execute(&client, request)
            .await
            .map_err(|e| TlError::Transport(e.to_string()))?;

        Ok(response)
    }
}
