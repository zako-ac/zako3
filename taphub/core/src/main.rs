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

fn generate_cert() -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
    let cert =
        rcgen::generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()]).unwrap();
    let cert_der = CertificateDer::from(cert.cert.der().to_vec());
    let key_der = PrivateKeyDer::try_from(cert.key_pair.serialize_der()).unwrap();
    (vec![cert_der], key_der)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    tracing::info!("Starting TapHub Core...");

    let config = AppConfig::load()?;
    let bind_addr = config.bind_addr.parse()?;

    let (cert_chain, private_key) = generate_cert();

    let app = Arc::new(App {
        hq_repository: Arc::new(StubHqRepository),
        cache_repository: Arc::new(StubCacheRepository),
    });

    let mut server = TransportServer::new(bind_addr, cert_chain, private_key, app)?;

    tracing::info!("Listening on {}", server.local_addr()?);
    server.run().await;

    Ok(())
}
