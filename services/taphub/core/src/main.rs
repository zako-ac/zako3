use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::process;
use std::sync::Arc;
use zako3_cache_client::RemoteAudioCache;
use zako3_metrics::TapRedisMetrics;
use zako3_preload_cache::AudioCache;
use zako3_states::{RedisCacheRepository, RedisPubSub, TapHubStateService};
use zako3_taphub_core::app::App;
use zako3_taphub_core::config::AppConfig;
use zako3_taphub_core::hub::TapHub;
use zako3_taphub_core::infra::hq::RpcHqRepository;
use zako3_taphub_transport_server::TransportServer;

use std::fs::File;
use std::io::BufReader;

fn load_certs(
    config: &AppConfig,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), Box<dyn std::error::Error>> {
    let cert_path = &config.transport_cert_file;
    let key_path = &config.transport_key_file;

    let cert_file = &mut BufReader::new(File::open(cert_path)?);
    let key_file = &mut BufReader::new(File::open(key_path)?);

    let cert_chain = rustls_pemfile::certs(cert_file).collect::<Result<Vec<_>, _>>()?;

    let private_key =
        rustls_pemfile::private_key(key_file)?.ok_or("No private key found in file")?;

    Ok((cert_chain, private_key))
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let config = AppConfig::load()?;

    let telemetry = zako3_telemetry::init(zako3_telemetry::TelemetryConfig {
        service_name: "taphub".to_string(),
        otlp_endpoint: config.otlp_endpoint.clone(),
        metrics_port: config.metrics_port,
    })
    .await?;

    tracing::info!("Starting TapHub Core...");

    if config.bypass_hq {
        tracing::warn!(
            "ZK_TH_BYPASS_HQ=true — accepting all tap authentications and audio requests without HQ. Do NOT run in production."
        );
    }

    let bind_addr = config.transport_bind_addr.parse()?;

    let (cert_chain, private_key) = load_certs(&config)?;

    let hq_repository = RpcHqRepository::new(&config.hq_rpc_url, &config.hq_rpc_admin_token)?;

    let cache_repo = Arc::new(RedisCacheRepository::new(&config.redis_url).await?);
    let app = App {
        hq_repository: Arc::new(hq_repository),
        cache_repository: cache_repo.clone(),
        tap_state_service: TapHubStateService::new(cache_repo.clone())
            .with_lease_ttl_secs(config.connection_lease_ttl_secs),
        tap_metrics_service: TapRedisMetrics::new(cache_repo.clone()),
        bypass_hq: config.bypass_hq,
    };

    // Stale connection state from a previous run self-heals via TTL leases: any
    // leftover `tap:{id}` key expires within ZK_TH_CONNECTION_LEASE_TTL_SECS, and
    // live taps reconnect and re-publish. No boot-time clear is needed.

    let history_pubsub = Arc::new(
        RedisPubSub::new(&config.redis_url)
            .await
            .expect("Failed to connect RedisPubSub"),
    );

    let cache_client = RemoteAudioCache::new(
        config.cache_rpc_url.clone(),
        config.cache_rpc_admin_token.clone(),
    )?;
    let audio_cache: Arc<dyn AudioCache> = Arc::new(cache_client);

    let tap_hub = TapHub::new(
        app.clone(),
        &config.zakofish_bind_addr,
        config.zakofish_bind_addr_pf3.as_deref(),
        &config.zakofish_cert_file,
        &config.zakofish_key_file,
        audio_cache,
        config.request_timeout_ms,
        history_pubsub,
    )
    .await?;
    let tap_hub = Arc::new(tap_hub);

    let tap_hub_clone = tap_hub.clone();
    tokio::spawn(async move {
        tracing::info!("Starting TapHub...");
        if let Err(e) = tap_hub_clone.run().await {
            tracing::error!(%e, "Error running TapHub");
            process::exit(1);
        }
    });

    let mut server = TransportServer::new(bind_addr, cert_chain, private_key, tap_hub)?;

    tracing::info!("Listening on {}", server.local_addr()?);
    telemetry.healthy();

    tokio::select! {
        _ = server.run() => {
            tracing::warn!("Transport server exited");
        }
        res = tokio::signal::ctrl_c() => {
            if let Err(e) = res {
                tracing::warn!(%e, "Failed to listen for Ctrl-C");
            }
            tracing::info!("Ctrl-C received, shutting down; dhat will dump dhat-heap.json on drop");
        }
    }

    // Drops `_profiler` here, which writes dhat-heap.json to the CWD.
    Ok(())
}
