use clap::{Args, Subcommand};

#[derive(Args)]
pub struct DebugCommands {
    #[command(subcommand)]
    pub command: DebugSubcommand,
}

#[derive(Subcommand)]
pub enum DebugSubcommand {
    /// Publish a PlayAudio history entry to the Redis history PubSub channel
    PublishHistory {
        /// Tap ID
        #[arg(long)]
        tap_id: String,
        /// Discord user ID
        #[arg(long)]
        discord_user_id: Option<String>,
        /// ARS length
        #[arg(long, default_value = "0")]
        ars_length: usize,
        /// Mark as cache hit
        #[arg(long, default_value_t = false)]
        cache_hit: bool,
        /// Mark as successful
        #[arg(long, default_value_t = true)]
        success: bool,
        /// Optional trace ID for dedup
        #[arg(long)]
        trace_id: Option<String>,
        /// Redis URL
        #[arg(long, env = "REDIS_URL", default_value = "redis://127.0.0.1:6379")]
        redis_url: String,
    },
}
