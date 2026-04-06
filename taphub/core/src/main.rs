use async_trait::async_trait;
use dashmap::DashMap;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::sync::Arc;
use tracing::Level;
use zako3_states::{
    CacheRepository, RedisCacheRepository, TapHubStateService, TapMetricsStateService,
};
use zako3_taphub_core::app::App;
use zako3_taphub_core::config::AppConfig;
use zako3_taphub_core::hub::TapHub;
use zako3_taphub_core::infra::hq::RpcHqRepository;
use zako3_taphub_core::repository::HqRepository;
use zako3_taphub_transport_server::TransportServer;
use zako3_types::hq::{
    DiscordUserId, ResourceTimestamp, Tap, TapId, TapOccupation, TapPermission, User, UserId,
    Username,
};

#[allow(dead_code)]
struct StubHqRepository;

#[async_trait]
impl HqRepository for StubHqRepository {
    async fn authenticate_tap(&self, _token: &str) -> Option<Tap> {
        Tap {
            id: TapId("0x67e55044_10b1_426f".to_string()),
            name: "mytap".to_string().into(),
            description: "This is a stub tap for testing".to_string().into(),
            owner_id: UserId("1".to_string()),
            occupation: TapOccupation::Base,
            permission: TapPermission::OwnerOnly,
            roles: vec![],
            base_volume: 1.0,
            timestamp: ResourceTimestamp::now(),
        }
        .into()
    }
    async fn get_tap_by_id(&self, _tap_id: &str) -> Option<Tap> {
        Tap {
            id: TapId("0x67e55044_10b1_426f".to_string()),
            name: "mytap".to_string().into(),
            description: "This is a stub tap for testing".to_string().into(),
            owner_id: UserId("1".to_string()),
            occupation: TapOccupation::Base,
            permission: TapPermission::OwnerOnly,
            roles: vec![],
            base_volume: 1.0,
            timestamp: ResourceTimestamp::now(),
        }
        .into()
    }

    async fn get_user_by_discord_id(&self, discord_id: &DiscordUserId) -> Option<User> {
        Some(User {
            id: UserId("1".to_string()),
            discord_user_id: discord_id.clone(),
            username: Username("stubuser".to_string()),
            avatar_url: None,
            email: None,
            permissions: vec![],
            timestamp: ResourceTimestamp::now(),
            banned: false,
        })
    }
}

#[allow(dead_code)]
#[derive(Default)]
struct StubCacheRepository {
    data: DashMap<String, String>,
    hll: DashMap<String, std::collections::HashSet<String>>,
}

#[async_trait]
impl CacheRepository for StubCacheRepository {
    async fn get(&self, key: &str) -> Option<String> {
        self.data.get(key).map(|v| v.value().clone())
    }
    async fn set(&self, key: &str, value: &str) {
        self.data.insert(key.to_string(), value.to_string());
    }

    async fn del(&self, key: &str) {
        self.data.remove(key);
    }

    async fn incr(&self, key: &str) -> Result<i64, zako3_states::StateServiceError> {
        let mut val = self
            .data
            .get(key)
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
        val += 1;
        self.data.insert(key.to_string(), val.to_string());
        Ok(val)
    }

    async fn decr(&self, key: &str) -> Result<i64, zako3_states::StateServiceError> {
        let mut val = self
            .data
            .get(key)
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
        val -= 1;
        self.data.insert(key.to_string(), val.to_string());
        Ok(val)
    }

    async fn incrby(&self, key: &str, amount: i64) -> Result<i64, zako3_states::StateServiceError> {
        let mut val = self
            .data
            .get(key)
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
        val += amount;
        self.data.insert(key.to_string(), val.to_string());
        Ok(val)
    }

    async fn pfadd(&self, key: &str, element: &str) -> Result<(), zako3_states::StateServiceError> {
        self.hll
            .entry(key.to_string())
            .or_default()
            .insert(element.to_string());
        Ok(())
    }

    async fn pfcount(&self, key: &str) -> Result<u64, zako3_states::StateServiceError> {
        Ok(self.hll.get(key).map(|v| v.len() as u64).unwrap_or(0))
    }

    async fn sadd(&self, key: &str, member: &str) -> Result<(), zako3_states::StateServiceError> {
        self.data
            .insert(format!("{}:set:{}", key, member), "true".to_string());
        Ok(())
    }

    async fn smembers(&self, key: &str) -> Result<Vec<String>, zako3_states::StateServiceError> {
        let prefix = format!("{}:set:", key);
        Ok(self
            .data
            .iter()
            .filter(|kv| kv.key().starts_with(&prefix))
            .map(|kv| kv.key().strip_prefix(&prefix).unwrap().to_string())
            .collect())
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

    let hq_repository = RpcHqRepository::new(&config.hq_rpc_url, &config.hq_rpc_admin_token)?;

    let cache_repo = Arc::new(RedisCacheRepository::new(&config.redis_url).await?);
    let app = App {
        hq_repository: Arc::new(hq_repository),
        cache_repository: cache_repo.clone(),
        tap_state_service: TapHubStateService::new(cache_repo.clone()),
        tap_metrics_service: TapMetricsStateService::new(cache_repo.clone()),
    };

    // Clear stale tap connection states left over from the previous run.
    // Taps that are still online will reconnect and re-register immediately.
    let known_taps = app.tap_metrics_service.get_known_taps().await.unwrap_or_default();
    if let Err(e) = app.tap_state_service.clear_all_tap_states(&known_taps).await {
        tracing::warn!(%e, "Failed to clear stale tap states on startup");
    }
    tracing::info!("Cleared online state for {} known taps", known_taps.len());

    let tap_hub = TapHub::new(
        app.clone(),
        &config.zakofish_bind_addr,
        &config.cert_file,
        &config.key_file,
        config.cache_dir.clone(),
        config.request_timeout_ms,
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
