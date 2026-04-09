use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use protofish2::compression::CompressionType;
use protofish2::connection::ClientConfig;
use tokio::sync::mpsc;
use zakofish::config::load_certs;
use zakofish::tap::ZakofishTap;
use zakofish::types::message::TapClientHello;
use zakofish::types::model::TapId;

use crate::error::SdkError;
use crate::handler::TapHandler;
use crate::source::AudioSource;
use crate::stream::AudioStreamSender;

pub fn tap() -> TapBuilder {
    TapBuilder::default()
}

#[derive(Default)]
pub struct TapBuilder {
    cert_pem: Option<PathBuf>,
    hub_addr: Option<String>,
    tap_id: Option<String>,
    friendly_name: Option<String>,
    api_token: Option<String>,
    selection_weight: f32,
}

impl TapBuilder {
    /// Path to the hub's root certificate PEM file.
    pub fn cert_pem(mut self, path: impl AsRef<Path>) -> Self {
        self.cert_pem = Some(path.as_ref().to_path_buf());
        self
    }

    /// Hub address as "host:port". The host part is also used as the TLS server name.
    pub fn hub(mut self, addr: impl Into<String>) -> Self {
        self.hub_addr = Some(addr.into());
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

    /// Connect to the Hub and block until the connection is permanently lost.
    /// Reconnection with exponential backoff is handled internally by zakofish.
    pub async fn run(self, handler: Arc<dyn TapHandler>) -> Result<(), SdkError> {
        let hub_addr = self
            .hub_addr
            .as_deref()
            .ok_or_else(|| SdkError::Tls("hub is required".to_string()))?;

        // Append default port 7060 if no port is present
        let hub_addr = if hub_addr
            .rsplit_once(':')
            .map(|(_, p)| p.parse::<u16>().is_ok())
            .unwrap_or(false)
        {
            hub_addr.to_string()
        } else {
            format!("{}:7060", hub_addr)
        };

        // Extract host as TLS server_name (SNI); resolve domain to SocketAddr
        let (server_name, _) = hub_addr
            .rsplit_once(':')
            .ok_or_else(|| SdkError::Tls("hub must be host:port".to_string()))?;
        let server_name = server_name.to_string();

        let socket_addr = tokio::net::lookup_host(&hub_addr)
            .await
            .map_err(SdkError::Io)?
            .next()
            .ok_or_else(|| SdkError::Tls(format!("could not resolve: {}", hub_addr)))?;

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

        let client_config = ClientConfig {
            bind_address: "127.0.0.1:0"
                .parse()
                .map_err(SdkError::AddrParse)?,
            root_certificates,
            supported_compression_types: vec![CompressionType::None],
            keepalive_range: Duration::from_secs(1)..Duration::from_secs(10),
            protofish_config: Default::default(),
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

        let bridge = Arc::new(HandlerBridge(handler));
        let zf_tap = ZakofishTap::new(client_config)?;
        zf_tap
            .connect_and_run(socket_addr, server_name.as_str(), hello, bridge)
            .await?;
        Ok(())
    }
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
            mpsc::Receiver<(protofish2::Timestamp, bytes::Bytes)>,
        ),
        zakofish::types::message::AudioRequestFailureMessage,
    > {
        let (tx, rx) = mpsc::channel(32);
        let sender = AudioStreamSender { tx };
        let source = AudioSource::from(ars);

        self.0
            .handle_audio_request(source, sender)
            .await
            .map(|success| (success, rx))
            .map_err(|e| e.into_wire())
    }
}
