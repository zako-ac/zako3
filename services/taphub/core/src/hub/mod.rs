use std::{
    collections::HashMap,
    path::Path,
    sync::Arc,
    time::Duration,
};

use parking_lot::Mutex;
use tokio::sync::watch;
use zako3_types::hq::TapId;
use zakofish_taphub::{
    ZakofishError,
    create_server_config,
    create_server_config_pf3,
    hub::ZakofishHub,
};

use zako3_preload_cache::AudioCache;

use crate::{app::App, routing::DynamicSampler};
use zako3_metrics::TapRedisMetrics;
use zako3_states::{RedisPubSub, TapHubStateService};

pub mod connection;
pub use connection::TapHubConnectionHandler;

pub(crate) type ConnectionSignals = Arc<Mutex<HashMap<u64, watch::Sender<bool>>>>;

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
    pub(crate) connection_signals: ConnectionSignals,
}

impl TapHub {
    pub async fn new(
        app: App,
        bind_address: &str,
        bind_address_pf3: Option<&str>,
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

        let server_config_pf3 = bind_address_pf3
            .map(|addr| {
                let parsed = addr.parse().map_err(|_| {
                    ZakofishError::ProtocolError(
                        "Invalid pf3 bind address is provided".to_string(),
                    )
                })?;
                create_server_config_pf3(parsed, &cert_file, &key_file)
            })
            .transpose()?;

        let connection_signals: ConnectionSignals = Arc::new(Mutex::new(HashMap::new()));

        let handler = TapHubConnectionHandler {
            app: app.clone(),
            state_service: app.tap_state_service.clone(),
            metrics_service: app.tap_metrics_service.clone(),
            connection_signals: Arc::clone(&connection_signals),
        };

        let zf_hub =
            ZakofishHub::new(Some(server_config), server_config_pf3, Arc::new(handler))?;

        Ok(Self {
            zf_hub,
            sampler: Arc::new(Mutex::new(DynamicSampler::new())),
            state_service: app.tap_state_service.clone(),
            metrics_service: app.tap_metrics_service.clone(),
            app,
            audio_cache,
            request_timeout: Duration::from_millis(request_timeout_ms),
            history_pubsub,
            connection_signals,
        })
    }

    pub async fn run(&self) -> Result<(), ZakofishError> {
        self.zf_hub.run().await
    }

    /// Select an available connection for a tap.
    pub(crate) async fn select_connection(
        &self,
        tap_id: &TapId,
    ) -> Result<(u64, watch::Receiver<bool>), zako3_types::TapHubError> {
        let states = self
            .state_service
            .get_tap_states(tap_id)
            .await
            .map_err(|e| {
                zako3_types::TapHubError::Internal(format!("Failed to get tap states: {}", e))
            })?;

        let available: Vec<_> = states.to_vec();

        let connection_id = self
            .sampler
            .lock()
            .next_connection_id(&available)
            .ok_or(zako3_types::TapHubError::TapUnavailable)?;

        // Subscribe to disconnect signal for this connection
        let disconnect_rx = self
            .connection_signals
            .lock()
            .get(&connection_id)
            .map(|tx| tx.subscribe())
            .unwrap_or_else(|| {
                // Connection already gone — treat as immediately disconnected
                let (_, rx) = watch::channel(true);
                rx
            });

        Ok((connection_id, disconnect_rx))
    }
}
