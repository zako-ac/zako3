use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use zakofish::config::load_certs;
use zakofish::tap_pf3::ZakofishTapPf3;
use zakofish::types::message::TapClientHello;
use zakofish::types::model::TapId;

use crate::error::SdkError;
use crate::handler::TapHandler;
use crate::source::AudioSource;
use crate::stream::AudioStreamSender;

/// Transport selection for [`TapBuilder`]. protofish3 is the only supported
/// transport; the enum is retained for source compatibility with taps that
/// call `.transport(Transport::Protofish3)`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Transport {
    #[default]
    Protofish3,
}

pub fn tap() -> TapBuilder {
    TapBuilder::default()
}

#[derive(Default)]
pub struct TapBuilder {
    cert_pem: Option<PathBuf>,
    hub_addr: Option<String>,
    server_name: Option<String>,
    tap_id: Option<String>,
    friendly_name: Option<String>,
    api_token: Option<String>,
    selection_weight: f32,
    transport: Transport,
    drain_timeout: Option<Duration>,
    #[cfg(feature = "healthcheck")]
    healthcheck_port: Option<u16>,
}

impl TapBuilder {
    /// Path to the hub's root certificate PEM file.
    pub fn cert_pem(mut self, path: impl AsRef<Path>) -> Self {
        self.cert_pem = Some(path.as_ref().to_path_buf());
        self
    }

    /// Hub address as "host:port". The host part is also used as the TLS server name if not
    /// specified separately. Default is api.zako.ac:7060.
    pub fn hub(mut self, addr: impl Into<String>) -> Self {
        self.hub_addr = Some(addr.into());
        self
    }

    pub fn server_name(mut self, name: impl Into<String>) -> Self {
        self.server_name = Some(name.into());
        self
    }

    pub fn tap_id(mut self, id: impl Into<String>) -> Self {
        self.tap_id = Some(id.into());
        self
    }

    pub fn friendly_name(mut self, name: impl Into<String>) -> Self {
        self.friendly_name = Some(name.into());
        self
    }

    pub fn api_token(mut self, token: impl Into<String>) -> Self {
        self.api_token = Some(token.into());
        self
    }

    pub fn selection_weight(mut self, weight: f32) -> Self {
        self.selection_weight = weight;
        self
    }

    /// Select the wire transport. Defaults to [`Transport::Pf2`].
    pub fn transport(mut self, transport: Transport) -> Self {
        self.transport = transport;
        self
    }

    /// How long a drain may keep answering requests that were already accepted
    /// before the process stops for good.
    ///
    /// The default is [`DEFAULT_DRAIN_TIMEOUT`], overridable with
    /// `ZAKO_DRAIN_TIMEOUT_SECS`. Whatever this is, the pod's
    /// `terminationGracePeriodSeconds` has to be comfortably longer, or the
    /// kubelet kills the tap in the middle of the drain it asked for.
    pub fn drain_timeout(mut self, timeout: Duration) -> Self {
        self.drain_timeout = Some(timeout);
        self
    }

    /// Spawn a minimal HTTP server on `port` that responds `200 OK` to `GET /health`.
    ///
    /// Requires the `healthcheck` crate feature. The server runs as a background
    /// task alongside the Hub connection; bind failures are logged but do not
    /// abort the tap.
    #[cfg(feature = "healthcheck")]
    pub fn healthcheck_port(mut self, port: u16) -> Self {
        self.healthcheck_port = Some(port);
        self
    }

    /// Connect to the Hub and block until the connection is permanently lost.
    /// Reconnection with exponential backoff is handled internally by zakofish.
    pub async fn run(self, handler: Arc<dyn TapHandler>) -> Result<(), SdkError> {
        let hub_addr = self.hub_addr.as_deref().unwrap_or("api.zako.ac:1028");

        // Append default pf3 port 1028 if no port is present
        let hub_addr = if hub_addr
            .rsplit_once(':')
            .map(|(_, p)| p.parse::<u16>().is_ok())
            .unwrap_or(false)
        {
            hub_addr.to_string()
        } else {
            format!("{}:1028", hub_addr)
        };

        // Extract host as TLS server_name (SNI); resolve domain to SocketAddr
        let server_name = self
            .server_name
            .clone()
            .unwrap_or_else(|| hub_addr.split(':').next().unwrap_or_default().to_string());

        // Use provided cert PEM or fall back to system trust store
        let root_certificates = if let Some(cert_path) = &self.cert_pem {
            load_certs(cert_path)?
        } else {
            let result = rustls_native_certs::load_native_certs();
            if !result.errors.is_empty() {
                tracing::warn!("some system certs failed to load: {:?}", result.errors);
            }
            result.certs
        };

        let hello = TapClientHello {
            tap_id: TapId::from_str(
                self.tap_id
                    .as_deref()
                    .ok_or_else(|| SdkError::Tls("tap_id is required".to_string()))?,
            )
            .map_err(|_| SdkError::Tls("invalid tap_id format".to_string()))?,
            friendly_name: self.friendly_name.unwrap_or_default(),
            api_token: self.api_token.unwrap_or_default(),
            selection_weight: self.selection_weight,
        };

        #[cfg(feature = "healthcheck")]
        if let Some(port) = self.healthcheck_port {
            tokio::spawn(crate::healthcheck::run_healthcheck_server(port));
        }

        let bridge = Arc::new(HandlerBridge(handler));

        let drain = zakofish::tap_pf3::DrainConfig {
            timeout: self.drain_timeout.unwrap_or_else(drain_timeout_from_env),
            handle_signals: true,
        };

        let mut client_config =
            protofish3::ClientConfig::new("0.0.0.0:0".parse().map_err(SdkError::AddrParse)?);
        client_config.root_certificates = root_certificates;
        client_config.protofish = zakofish::default_protofish3_config();
        client_config.handshake_timeout = Duration::from_secs(10);

        let zf_tap = ZakofishTapPf3::new(client_config)?;
        zf_tap
            .connect_and_run_with(&hub_addr, server_name.as_str(), hello, bridge, drain)
            .await?;

        Ok(())
    }
}

/// How long a drain may run when the caller did not say.
///
/// Above the slowest single request measured on any tap (supertonic synthesis:
/// p99 635ms, worst 753ms over a day of traffic) with room for a request that
/// started just before the drain did, and well inside the 45s grace period the
/// tap manifests give the pod.
pub const DEFAULT_DRAIN_TIMEOUT: Duration = zakofish::tap_pf3::DrainConfig::DEFAULT_TIMEOUT;

fn drain_timeout_from_env() -> Duration {
    std::env::var("ZAKO_DRAIN_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|secs| *secs > 0)
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_DRAIN_TIMEOUT)
}

struct HandlerBridge(Arc<dyn TapHandler>);

#[async_trait::async_trait]
impl zakofish::tap::TapHandler for HandlerBridge {
    async fn handle_audio_metadata_request(
        &self,
        ars: zakofish::types::model::AudioRequestString,
        _headers: HashMap<String, String>,
    ) -> std::result::Result<
        zakofish::types::message::AudioMetadataSuccessMessage,
        zakofish::types::message::AudioRequestFailureMessage,
    > {
        self.0
            .handle_audio_metadata_request(AudioSource::from(ars))
            .await
            .map_err(|e| e.into_wire())
    }

    async fn handle_audio_request(
        &self,
        ars: zakofish::types::model::AudioRequestString,
        _headers: HashMap<String, String>,
    ) -> std::result::Result<
        (
            zakofish::types::message::AudioRequestSuccessMessage,
            mpsc::Receiver<(zakofish::Timestamp, bytes::Bytes)>,
            zakofish::TransferMode,
        ),
        zakofish::types::message::AudioRequestFailureMessage,
    > {
        let (tx, rx) = mpsc::channel(32);
        let transfer_mode = Arc::new(std::sync::OnceLock::new());
        let sender = AudioStreamSender {
            tx,
            transfer_mode: transfer_mode.clone(),
        };
        let source = AudioSource::from(ars);

        self.0
            .handle_audio_request(source, sender)
            .await
            .map(|success| {
                let mode = transfer_mode
                    .get()
                    .copied()
                    .unwrap_or(zakofish::TransferMode::Dual);
                (success, rx, mode)
            })
            .map_err(|e| e.into_wire())
    }
}
