use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "cache-gc", about = "Audio cache garbage collector")]
pub struct Cli {
    /// Cache directory containing cache.db and .opus files
    #[arg(long, env = "CACHE_DIR")]
    pub cache_dir: PathBuf,

    /// Maximum total cache size in bytes (required for evict-gdsf / run-all)
    #[arg(long, env = "MAX_CACHE_BYTES")]
    pub max_bytes: Option<u64>,

    /// GDSF eviction batch size
    #[arg(long, env = "GC_BATCH_SIZE", default_value = "50")]
    pub batch_size: usize,

    /// Redis URL for persisting metrics (optional)
    #[arg(long, env = "REDIS_URL")]
    pub redis_url: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Remove all expired cache entries
    EvictExpired,
    /// Remove orphan .opus files and ghost DB rows
    EvictDangling,
    /// Evict by GDSF priority until total size is under --max-bytes
    EvictGdsf,
    /// Probe all .opus files with Symphonia; evict corrupt ones
    Validate,
    /// Run: evict-expired → evict-dangling → evict-gdsf (no validation)
    RunEvict,
    /// Run: evict-expired → evict-dangling → evict-gdsf → validate
    RunAll,
}
