mod actions;
mod config;
mod metrics;
mod server;

use std::sync::Arc;

use anyhow::{Context, Result};
use zako3_preload_cache::{AudioPreload, FileAudioCache};

use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;

    let telemetry = zako3_telemetry::init(zako3_telemetry::TelemetryConfig {
        service_name: "cache".to_string(),
        otlp_endpoint: config.otlp_endpoint.clone(),
        metrics_port: config.metrics_port,
    })
    .await?;

    // Optional Redis connection for metrics persistence.
    let cache_repo: Option<Arc<zako3_states::RedisCacheRepository>> =
        if let Some(url) = config.redis_url.as_deref() {
            let repo = zako3_states::RedisCacheRepository::new(url)
                .await
                .context("failed to connect to Redis")?;
            Some(Arc::new(repo))
        } else {
            None
        };

    let cache = Arc::new(
        FileAudioCache::open(config.cache_dir.clone(), None)
            .await
            .context("failed to open FileAudioCache")?,
    );
    let preload = Arc::new(AudioPreload::new(config.cache_dir.clone(), None));

    server::gc::spawn(
        server::gc::GcConfig {
            interval: config.gc.interval,
            max_bytes: config.gc.max_bytes,
            batch_size: config.gc.batch_size,
        },
        Arc::clone(&cache),
        config.cache_dir.clone(),
        cache_repo.clone(),
    );

    let router = server::build(
        Arc::clone(&cache),
        Arc::clone(&preload),
        config.admin_token.clone(),
    );
    let addr: std::net::SocketAddr = config.bind_addr.parse()?;
    telemetry.healthy();

    tokio::select! {
        res = server::serve(addr, router) => {
            if let Err(e) = res {
                tracing::error!(%e, "cache server exited with error");
                return Err(e);
            }
        }
        res = tokio::signal::ctrl_c() => {
            if let Err(e) = res {
                tracing::warn!(%e, "failed to listen for Ctrl-C");
            }
            tracing::info!("Ctrl-C received, shutting down cache server");
        }
    }

    Ok(())
}
