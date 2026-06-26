pub mod cli;

use anyhow::Result;
use cli::{DebugCommands, DebugSubcommand};
use zako3_states::RedisPubSub;
use zako3_types::hq::{
    DiscordUserId, TapId,
    history::{PlayAudioHistory, UseHistoryEntry},
};

pub async fn handle_command(cmd: DebugCommands) -> Result<()> {
    match cmd.command {
        DebugSubcommand::PublishHistory {
            tap_id,
            discord_user_id,
            ars_length,
            cache_hit,
            success,
            trace_id,
            redis_url,
        } => {
            let entry = UseHistoryEntry::PlayAudio(PlayAudioHistory {
                user_id: None,
                discord_user_id: discord_user_id.map(DiscordUserId),
                ars_length,
                trace_id,
                tap_id: TapId(tap_id),
                cache_hit,
                success,
            });
            let pubsub = RedisPubSub::new(&redis_url).await?;
            pubsub.publish_history(&entry).await?;
            println!("Published history entry to Redis 'history' channel.");
            Ok(())
        }
    }
}
