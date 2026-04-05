use async_trait::async_trait;
use dashmap::DashMap;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::sync::Arc;
use tracing::Level;
use uuid::Uuid;
use zako3_taphub_core::app::App;
use zako3_taphub_core::config::AppConfig;
use zako3_taphub_core::hub::TapHub;
use zako3_taphub_core::infra::hq::RpcHqRepository;
use zako3_taphub_core::repository::{CacheRepository, HqRepository};
use zako3_taphub_transport_server::TransportServer;
use zako3_types::hq::{ResourceTimestamp, Tap, TapOccupation, TapPermission};

#[allow(dead_code)]
struct StubHqRepository;

#[async_trait]
impl HqRepository for StubHqRepository {
    async fn authenticate_tap(&self, _token: &str) -> Option<Tap> {
        Tap {
            id: Uuid::from_u128(0x67e55044_10b1_426f_9247_bb680e5fe0c8).into(),
            name: "mytap".to_string().into(),
            description: "This is a stub tap for testing".to_string().into(),
            owner_id: Uuid::new_v4().into(),
            occupation: TapOccupation::Base,
            permission: TapPermission::OwnerOnly,
            role: None,
            timestamp: ResourceTimestamp::now(),
        }
        .into()
    }
    async fn get_tap(&self, _tap_id: &str) -> Option<Tap> {
        Tap {
            id: Uuid::from_u128(0x67e55044_10b1_426f_9247_bb680e5fe0c8).into(),
            name: "mytap".to_string().into(),
            description: "This is a stub tap for testing".to_string().into(),
            owner_id: Uuid::new_v4().into(),
            occupation: TapOccupation::Base,
            permission: TapPermission::OwnerOnly,
            role: None,
            timestamp: ResourceTimestamp::now(),
        }
        .into()
    }
}

#[derive(Default)]
struct StubCacheRepository {
    data: DashMap<String, String>,
}

#[async_trait]
impl CacheRepository for StubCacheRepository {
    async fn get(&self, key: &str) -> Option<String> {
        self.data.get(key).map(|v| v.value().clone())
    }
    async fn set(&self, key: &str, value: &str) {
        self.data.insert(key.to_string(), value.to_string());
    }
}

use std::fs::File;
use std::io::BufReader;

fn load_certs(
    config: &AppConfig,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), Box<dyn std::error::Error>> {
    let cert_path = &config.cert_file;
    let key_path = &config.key_file;

    let cert_file = &mut BufReader::new(File::open(&cert_path)?);
    let key_file = &mut BufReader::new(File::open(&key_path)?);

    let cert_chain = rustls_pemfile::certs(cert_file).collect::<Result<Vec<_>, _>>()?;

    let private_key =
        rustls_pemfile::private_key(key_file)?.ok_or("No private key found in file")?;

    Ok((cert_chain, private_key))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    tracing::info!("Starting TapHub Core...");

    let config = AppConfig::load()?;
    let bind_addr = config.transport_bind_addr.parse()?;

    let (cert_chain, private_key) = load_certs(&config)?;

    let hq_repository = RpcHqRepository::new(&config.hq_rpc_url)?;

    let app = App {
        hq_repository: Arc::new(hq_repository),
        cache_repository: Arc::new(StubCacheRepository::default()),
    };

    let tap_hub = TapHub::new(
        app.clone(),
        &config.zakofish_bind_addr,
        &config.cert_file,
        &config.key_file,
    )?;
    let tap_hub = Arc::new(tap_hub);

    let tap_hub_clone = tap_hub.clone();
    tokio::spawn(async move {
        tracing::info!("Starting TapHub...");
        if let Err(e) = tap_hub_clone.run().await {
            tracing::error!(%e, "Error running TapHub");
        }
    });

    let mut server = TransportServer::new(bind_addr, cert_chain, private_key, tap_hub)?;

    tracing::info!("Listening on {}", server.local_addr()?);
    server.run().await;

    Ok(())
}
