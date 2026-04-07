mod actions;
mod config;
mod metrics;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::Level;
use zako3_preload_cache::FileAudioCache;

use config::{Cli, Command};
use metrics::ActionMetrics;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let cli = Cli::parse();

    // Optional Redis connection for metrics persistence.
    let cache_repo: Option<zako3_states::RedisCacheRepository> = if let Some(ref url) = cli.redis_url {
        let repo = zako3_states::RedisCacheRepository::new(url)
            .await
            .context("failed to connect to Redis")?;
        Some(repo)
    } else {
        None
    };

    let cache = FileAudioCache::open(cli.cache_dir.clone(), None)
        .await
        .context("failed to open FileAudioCache")?;

    match cli.command {
        Command::EvictExpired => {
            let m = actions::expired::evict_expired(&cache).await?;
            finish(m, cache_repo.as_ref()).await;
        }
        Command::EvictDangling => {
            let m = actions::dangling::evict_dangling(&cache, &cli.cache_dir).await?;
            finish(m, cache_repo.as_ref()).await;
        }
        Command::EvictGdsf => {
            let max_bytes = cli
                .max_bytes
                .context("--max-bytes is required for evict-gdsf")?;
            let m = actions::gdsf::evict_gdsf(&cache, max_bytes, cli.batch_size).await?;
            finish(m, cache_repo.as_ref()).await;
        }
        Command::Validate => {
            let m = actions::validate::validate_opus(&cache).await?;
            finish(m, cache_repo.as_ref()).await;
        }
        Command::RunEvict => {
            let max_bytes = cli
                .max_bytes
                .context("--max-bytes is required for run-evict")?;

            let m = actions::expired::evict_expired(&cache).await?;
            finish(m, cache_repo.as_ref()).await;

            let m = actions::dangling::evict_dangling(&cache, &cli.cache_dir).await?;
            finish(m, cache_repo.as_ref()).await;

            let m = actions::gdsf::evict_gdsf(&cache, max_bytes, cli.batch_size).await?;
            finish(m, cache_repo.as_ref()).await;
        }
        Command::RunAll => {
            let max_bytes = cli
                .max_bytes
                .context("--max-bytes is required for run-all")?;

            let m = actions::expired::evict_expired(&cache).await?;
            finish(m, cache_repo.as_ref()).await;

            let m = actions::dangling::evict_dangling(&cache, &cli.cache_dir).await?;
            finish(m, cache_repo.as_ref()).await;

            let m = actions::gdsf::evict_gdsf(&cache, max_bytes, cli.batch_size).await?;
            finish(m, cache_repo.as_ref()).await;

            let m = actions::validate::validate_opus(&cache).await?;
            finish(m, cache_repo.as_ref()).await;
        }
    }

    Ok(())
}

async fn finish(m: ActionMetrics, repo: Option<&zako3_states::RedisCacheRepository>) {
    m.log();
    if let Some(repo) = repo
        && let Err(e) = metrics::persist(&m, repo).await
    {
        tracing::warn!(%e, "failed to persist metrics to Redis");
    }
}
