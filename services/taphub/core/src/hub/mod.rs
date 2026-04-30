use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use async_nats::Client as NatsClient;

use parking_lot::Mutex;
use tokio::sync::watch;
use zako3_types::hq::TapId;
use zakofish_taphub::{
    ZakofishError,
    create_server_config,
    hub::ZakofishHub,
};

use zako3_preload_cache::{AudioPreload, FileAudioCache};

use crate::{app::App, routing::DynamicSampler};
use zako3_states::{RedisPubSub, TapHubStateService, TapMetricsStateService};

pub mod connection;
pub use connection::TapHubConnectionHandler;

pub(crate) type ConnectionSignals = Arc<Mutex<HashMap<u64, watch::Sender<bool>>>>;

pub struct TapHub {
    pub zf_hub: ZakofishHub,
    pub sampler: Arc<Mutex<DynamicSampler>>,
    pub state_service: TapHubStateService,
    pub metrics_service: TapMetricsStateService,
    pub app: App,
    pub audio_preload: Arc<AudioPreload>,
    pub audio_cache: Arc<FileAudioCache>,
    pub request_timeout: Duration,
    pub nats_client: Option<Arc<NatsClient>>,
    pub history_pubsub: Arc<RedisPubSub>,
    pub(crate) connection_signals: ConnectionSignals,
}

impl TapHub {
    pub async fn new(
        app: App,
        bind_address: &str,
        cert_file: impl AsRef<Path>,
        key_file: impl AsRef<Path>,
        cache_dir: PathBuf,
        request_timeout_ms: u64,
        nats_client: Option<Arc<NatsClient>>,
        history_pubsub: Arc<RedisPubSub>,
    ) -> Result<Self, ZakofishError> {
        let server_config = create_server_config(
            bind_address.parse().map_err(|_| {
                ZakofishError::ProtocolError("Invalid bind address is provided".to_string())
            })?,
            cert_file,
            key_file,
        )?;

        let connection_signals: ConnectionSignals = Arc::new(Mutex::new(HashMap::new()));

        let handler = TapHubConnectionHandler {
            app: app.clone(),
            state_service: app.tap_state_service.clone(),
            metrics_service: app.tap_metrics_service.clone(),
            connection_signals: Arc::clone(&connection_signals),
        };

        let zf_hub = ZakofishHub::new(server_config, Arc::new(handler))?;

        let audio_cache = FileAudioCache::open(cache_dir.clone(), None)
            .await
            .map_err(|e| ZakofishError::ProtocolError(e.to_string()))?;

        Ok(Self {
            zf_hub,
            sampler: Arc::new(Mutex::new(DynamicSampler::new())),
            state_service: app.tap_state_service.clone(),
            metrics_service: app.tap_metrics_service.clone(),
            app,
            audio_preload: Arc::new(AudioPreload::new(cache_dir, None)),
            audio_cache: Arc::new(audio_cache),
            request_timeout: Duration::from_millis(request_timeout_ms),
            nats_client,
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
    ) -> Result<(u64, watch::Receiver<bool>), String> {
        let states = self
            .state_service
            .get_tap_states(tap_id)
            .await
            .map_err(|e| format!("Failed to get tap states: {}", e))?;

        let available: Vec<_> = states.to_vec();

        let connection_id = self
            .sampler
            .lock()
            .next_connection_id(&available)
            .ok_or_else(|| "No available connections for this tap".to_string())?;

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
