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
            match tokio::time::timeout(
                std::time::Duration::from_secs(5),
                zako3_states::RedisCacheRepository::new(url),
            )
            .await
            {
                Ok(Ok(repo)) => Some(Arc::new(repo)),
                Ok(Err(e)) => {
                    tracing::warn!(%e, "Redis connect failed; continuing without cache repo");
                    None
                }
                Err(_) => {
                    tracing::warn!("Redis connect timed out; continuing without cache repo");
                    None
                }
            }
        } else {
            None
        };

    // Opened without scanning: on a large cache that scan takes minutes, and doing
    // it here would keep the listener closed long enough for the liveness probe to
    // kill the pod. Warmed in the background below; until then, lookups miss.
    let cache = Arc::new(
        FileAudioCache::open_empty(config.cache_dir.clone(), None)
            .await
            .context("failed to open FileAudioCache")?,
    );
    let preload = Arc::new(AudioPreload::new(config.cache_dir.clone(), None));
    let warmup = Arc::new(server::WarmupState::new());

    server::gc::spawn(
        server::gc::GcConfig {
            interval: config.gc.interval,
            max_bytes: config.gc.max_bytes,
            batch_size: config.gc.batch_size,
        },
        Arc::clone(&cache),
        config.cache_dir.clone(),
        cache_repo.clone(),
        Arc::clone(&warmup),
    );

    let router = server::build(
        Arc::clone(&cache),
        Arc::clone(&preload),
        config.admin_token.clone(),
        Arc::clone(&warmup),
    );
    let addr: std::net::SocketAddr = config.bind_addr.parse()?;
    let listener = server::bind(addr).await?;
    telemetry.healthy();

    tokio::spawn({
        let cache = Arc::clone(&cache);
        let warmup = Arc::clone(&warmup);
        let concurrency = config.warmup_concurrency;
        async move {
            let progress = |scanned, total| warmup.record_progress(scanned, total);
            match cache.warm_with_progress(concurrency, progress).await {
                Ok(_) => {}
                Err(e) => tracing::error!(%e, "cache index warmup failed; serving a partial index"),
            }
            warmup.mark_ready();
        }
    });

    tokio::select! {
        res = server::serve_on(listener, router) => {
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
