use std::{
    collections::HashMap,
    path::Path,
    sync::Arc,
    time::Duration,
};

use parking_lot::Mutex;
use tokio::sync::watch;
use zako3_types::{OnlineTapState, OnlineTapStates, hq::TapId};
use zakofish_taphub::{ZakofishError, create_server_config, hub::ZakofishHub};

use zako3_preload_cache::AudioCache;

use crate::{app::App, routing::DynamicSampler};
use zako3_metrics::TapRedisMetrics;
use zako3_states::{RedisPubSub, TapHubStateService};

pub mod connection;
pub use connection::TapHubConnectionHandler;

/// A single live tap connection held by this taphub instance.
///
/// The registry of these entries is the authoritative source of truth for which
/// connections exist — routing reads it directly (never Redis), so it cannot
/// drift. Redis holds only a TTL-leased projection for cross-service consumers.
pub(crate) struct ConnEntry {
    pub(crate) state: OnlineTapState,
    pub(crate) disconnect_tx: watch::Sender<bool>,
}

pub(crate) type ConnectionRegistry = Arc<Mutex<HashMap<u64, ConnEntry>>>;

pub struct TapHub {
    pub zf_hub: ZakofishHub,
    pub sampler: Arc<Mutex<DynamicSampler>>,
    pub state_service: TapHubStateService,
    pub metrics_service: TapRedisMetrics,
    pub app: App,
    /// Cache client. Today this is the HTTP-backed [`zako3_cache_client::RemoteAudioCache`];
    /// the trait indirection lets tests inject an in-memory impl.
    pub audio_cache: Arc<dyn AudioCache>,
    pub request_timeout: Duration,
    pub history_pubsub: Arc<RedisPubSub>,
    pub(crate) connections: ConnectionRegistry,
}

impl TapHub {
    pub async fn new(
        app: App,
        bind_address: &str,
        cert_file: impl AsRef<Path>,
        key_file: impl AsRef<Path>,
        audio_cache: Arc<dyn AudioCache>,
        request_timeout_ms: u64,
        history_pubsub: Arc<RedisPubSub>,
    ) -> Result<Self, ZakofishError> {
        let cert_file = cert_file.as_ref().to_path_buf();
        let key_file = key_file.as_ref().to_path_buf();

        let server_config = create_server_config(
            bind_address.parse().map_err(|_| {
                ZakofishError::ProtocolError("Invalid bind address is provided".to_string())
            })?,
            &cert_file,
            &key_file,
        )?;

        let connections: ConnectionRegistry = Arc::new(Mutex::new(HashMap::new()));

        let handler = TapHubConnectionHandler {
            app: app.clone(),
            state_service: app.tap_state_service.clone(),
            metrics_service: app.tap_metrics_service.clone(),
            connections: Arc::clone(&connections),
        };

        let zf_hub = ZakofishHub::new(server_config, Arc::new(handler))?;

        Ok(Self {
            zf_hub,
            sampler: Arc::new(Mutex::new(DynamicSampler::new())),
            state_service: app.tap_state_service.clone(),
            metrics_service: app.tap_metrics_service.clone(),
            app,
            audio_cache,
            request_timeout: Duration::from_millis(request_timeout_ms),
            history_pubsub,
            connections,
        })
    }

    pub async fn run(&self) -> Result<(), ZakofishError> {
        tokio::select! {
            r = self.zf_hub.run() => r,
            _ = self.lease_heartbeat() => Ok(()),
        }
    }

    /// Periodically re-publish every locally-held connection's state to Redis so
    /// its TTL lease never lapses while the connection is live. The registry is
    /// the source of truth; this only refreshes the projection. On a hard crash
    /// the loop stops, the keys expire, and stale "online" state disappears.
    async fn lease_heartbeat(&self) {
        let interval_secs = (self.state_service.lease_ttl_secs() / 3).max(1);
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;

            // Snapshot the registry grouped by tap, dropping the lock before any await.
            let by_tap: HashMap<TapId, OnlineTapStates> = {
                let guard = self.connections.lock();
                let mut map: HashMap<TapId, OnlineTapStates> = HashMap::new();
                for entry in guard.values() {
                    map.entry(entry.state.tap_id.clone())
                        .or_default()
                        .push(entry.state.clone());
                }
                map
            };

            for (tap_id, states) in by_tap {
                if let Err(e) = self.state_service.publish_tap_states(&tap_id, &states).await {
                    tracing::warn!(%e, tap_id = %tap_id.0, "Failed to refresh tap connection lease");
                }
            }
        }
    }

    /// Select an available connection for a tap.
    ///
    /// Reads the in-memory registry directly — the authoritative set of live
    /// connections — so a connection that has already been dropped is never
    /// handed out. The sampler picks a connection, then we subscribe to that
    /// connection's disconnect signal under the same logical view.
    pub(crate) async fn select_connection(
        &self,
        tap_id: &TapId,
    ) -> Result<(u64, watch::Receiver<bool>), zako3_types::TapHubError> {
        let guard = self.connections.lock();

        let available: OnlineTapStates = guard
            .values()
            .filter(|e| &e.state.tap_id == tap_id)
            .map(|e| e.state.clone())
            .collect();

        let connection_id = self
            .sampler
            .lock()
            .next_connection_id(&available)
            .ok_or(zako3_types::TapHubError::TapUnavailable)?;

        let disconnect_rx = guard
            .get(&connection_id)
            .map(|e| e.disconnect_tx.subscribe())
            .ok_or(zako3_types::TapHubError::TapUnavailable)?;

        Ok((connection_id, disconnect_rx))
    }
}
