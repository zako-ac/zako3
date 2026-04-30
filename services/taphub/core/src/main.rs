use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::process;
use std::sync::Arc;
use zako3_states::{RedisCacheRepository, RedisPubSub, TapHubStateService, TapMetricsStateService};
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
    let known_taps = match app.tap_metrics_service.get_known_taps().await {
        Ok(taps) => taps,
        Err(e) => {
            tracing::warn!(%e, "Failed to fetch known taps; skipping stale state clear");
            vec![]
        }
    };
    if let Err(e) = app
        .tap_state_service
        .clear_all_tap_states(&known_taps)
        .await
    {
        tracing::warn!(%e, "Failed to clear stale tap states on startup");
    }
    tracing::info!("Cleared online state for {} known taps", known_taps.len());

    let history_pubsub = Arc::new(
        RedisPubSub::new(&config.redis_url)
            .await
            .expect("Failed to connect RedisPubSub"),
    );

    let tap_hub = TapHub::new(
        app.clone(),
        &config.zakofish_bind_addr,
        &config.zakofish_cert_file,
        &config.zakofish_key_file,
        config.cache_dir.clone(),
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
    server.run().await;

    Ok(())
}
