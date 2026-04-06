use anyhow::{Context, Result};
use zako3_audio_engine_client::client::AudioEngineRpcClient;
use zako3_types::{
    AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName, TapName, TrackId, UserId,
    Volume, hq::DiscordUserId,
};

use crate::config::Config;
use crate::services::audio_engine::cli::{AudioEngineCommands, AudioEngineSubcommands};
use crate::services::audio_engine::formatter;

pub async fn handle_command(ae_addr: String, cmd: AudioEngineCommands) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    
    let endpoint = if ae_addr.starts_with("nats") {
        ae_addr
    } else if let Some(ctx) = config.get_active_context() {
        if ctx.ae_addr.starts_with("nats") {
            ctx.ae_addr.clone()
        } else {
            "nats://127.0.0.1:4222".to_string()
        }
    } else {
        "nats://127.0.0.1:4222".to_string()
    };

    println!("Connecting to Audio Engine at {}...", endpoint);
    let client = AudioEngineRpcClient::new(&endpoint).await.context("Failed to connect to NATS")?;

    // Helper closure to resolve guild_id from option or context
    let resolve_guild_id = |gid: Option<String>| -> Result<GuildId> {
        let id: u64 = if let Some(id) = gid {
            config.resolve_alias(&id).parse()?
        } else if let Some(ctx) = config.get_active_context() {
            if let Some(ref default_id) = ctx.default_guild_id {
                config.resolve_alias(default_id).parse()?
            } else {
                anyhow::bail!("Guild ID not provided and no default found in current context")
            }
        } else {
            anyhow::bail!("Guild ID not provided and no active context found")
        };
        Ok(GuildId::from(id))
    };

    match cmd.command {
        AudioEngineSubcommands::Join {
            guild_id,
            channel_id,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let success = client.join(gid, cid).await?;
            println!("Join Success: {}", success);
        }
        AudioEngineSubcommands::Leave { guild_id, channel_id } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let success = client.leave(gid, cid).await?;
            println!("Leave Success: {}", success);
        }
        AudioEngineSubcommands::Play {
            guild_id,
            channel_id,
            queue,
            tap,
            request,
            volume,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let track_id = client
                .play(
                    gid,
                    cid,
                    QueueName::from(queue),
                    TapName::from(tap),
                    AudioRequestString::from(config.resolve_alias(&request)),
                    Volume::from(volume),
                    DiscordUserId::from("".to_string()),
                )
                .await?;
            println!("Track ID: {}", track_id);
        }
        AudioEngineSubcommands::SetVolume {
            guild_id,
            channel_id,
            track_id,
            volume,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let success = client
                .set_volume(gid, cid, TrackId::from(track_id), Volume::from(volume))
                .await?;
            println!("SetVolume Success: {}", success);
        }
        AudioEngineSubcommands::Stop { guild_id, channel_id, track_id } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let tid = track_id.parse::<u64>().context("Invalid track ID")?;
            let success = client.stop(gid, cid, TrackId::from(tid)).await?;
            println!("Stop Success: {}", success);
        }
        AudioEngineSubcommands::StopMany {
            guild_id,
            channel_id,
            filter,
            user_id,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let filter_type = match filter.to_lowercase().as_str() {
                "all" => AudioStopFilter::All,
                "music" => AudioStopFilter::Music,
                "tts" => {
                    let uid = user_id.context("user_id is required for tts filter")?;
                    AudioStopFilter::TTS(UserId::from(uid.to_string()))
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Invalid filter type. Options: all, music, tts"
                    ));
                }
            };

            let success = client.stop_many(gid, cid, filter_type).await?;
            println!("StopMany Success: {}", success);
        }
        AudioEngineSubcommands::NextMusic { guild_id, channel_id } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let success = client.next_music(gid, cid).await?;
            println!("NextMusic Success: {}", success);
        }
        AudioEngineSubcommands::GetSessionState { guild_id, channel_id } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let state = client.get_session_state(gid, cid).await?;
            formatter::print_session_state_native(state);
        }
    }

    Ok(())
}
