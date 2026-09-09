// Only `config` is private to the binary; everything else comes from the lib,
// so the module tree is compiled once rather than once per target.
mod config;

use std::sync::Arc;

use anyhow::{Context, Result};
use zako3_cache::server;
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

    // Bound before the process reports healthy, so a port that cannot be opened
    // is a startup failure rather than a feature that silently went missing.
    let ingest = match &config.udp {
        Some(udp) => Some(server::ingest::start(udp.bind_addr.parse()?, udp.max_sessions).await?),
        None => None,
    };

    // Built here rather than inside the router, because the GC sweep, the
    // session reaper and the UDP pump all need the same session maps the
    // handlers use.
    let mut state = server::AppState::new(
        Arc::clone(&cache),
        Arc::clone(&preload),
        config.admin_token.clone(),
        Arc::clone(&warmup),
    );
    if let Some(ingest) = ingest {
        state = state.with_ingest(ingest);
    }

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
        state.clone(),
    );

    server::preload::reaper::spawn(state.clone(), config.preload_session_ttl);

    // Advertise where taps should send. HQ reads this to fill `deliver_to`;
    // without it the preload path has no address to hand out and falls back to
    // the legacy route, which is the right failure but a quiet one.
    if let (Some(udp), Some(repo)) = (&config.udp, &cache_repo) {
        let registry = zako3_states::SinkRegistry::new(Arc::clone(repo) as _);
        let ad = zako3_states::SinkAdvertisement {
            sink_id: udp.sink_id.clone(),
            kind: zako3_states::SinkKind::Cache,
            internal_addr: udp.internal_addr.clone(),
            public_addr: udp.public_addr.clone(),
        };
        let interval = std::time::Duration::from_secs((registry.lease_ttl_secs() / 3).max(1));
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                if let Err(e) = registry.advertise(&ad).await {
                    tracing::warn!(%e, sink_id = %ad.sink_id, "failed to advertise the cache sink");
                }
            }
        });
    } else if config.udp.is_some() {
        tracing::warn!("UDP ingest is configured but Redis is not; HQ cannot discover this sink");
    }

    let router = server::build(state);
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
