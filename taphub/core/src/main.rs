use async_trait::async_trait;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::sync::Arc;
use zako3_taphub_core::app::App;
use zako3_taphub_core::config::AppConfig;
use zako3_taphub_core::repository::{CacheRepository, HqRepository};
use zako3_taphub_transport_server::TransportServer;
use zako3_types::hq::Tap;

struct StubHqRepository;

#[async_trait]
impl HqRepository for StubHqRepository {
    async fn authenticate_tap(&self, _token: &str) -> Option<Tap> {
        None
    }
    async fn get_tap(&self, _tap_id: &str) -> Option<Tap> {
        None
    }
}

struct StubCacheRepository;

#[async_trait]
impl CacheRepository for StubCacheRepository {
    async fn get(&self, _key: &str) -> Option<String> {
        None
    }
    async fn set(&self, _key: &str, _value: &str) {}
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
    tracing_subscriber::fmt::init();

    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    tracing::info!("Starting TapHub Core...");

    let config = AppConfig::load()?;
    let bind_addr = config.bind_addr.parse()?;

    let (cert_chain, private_key) = load_certs(&config)?;

    let app = Arc::new(App {
        hq_repository: Arc::new(StubHqRepository),
        cache_repository: Arc::new(StubCacheRepository),
    });

    let mut server = TransportServer::new(bind_addr, cert_chain, private_key, app)?;

    tracing::info!("Listening on {}", server.local_addr()?);
    server.run().await;

    Ok(())
}
