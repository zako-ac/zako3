use anyhow::{Context, Result};
use zako3_tl_client::TlClient;
use zako3_types::{
    AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName, TrackId, UserId,
    Volume, hq::{DiscordUserId, TapId},
};

use crate::config::Config;
use crate::services::audio_engine::cli::{AudioEngineCommands, AudioEngineSubcommands};
use crate::services::audio_engine::formatter;

pub async fn handle_command(ae_addr: String, cmd: AudioEngineCommands) -> Result<()> {
    let config = Config::load().unwrap_or_default();

    let endpoint = if !ae_addr.is_empty() {
        ae_addr
    } else if let Some(ctx) = config.get_active_context() {
        ctx.ae_addr.clone()
    } else {
        "127.0.0.1:7070".to_string()
    };

    println!("Connecting to Traffic Light at {}...", endpoint);
    let client = TlClient::connect(&endpoint)
        .await
        .context("Failed to connect to Traffic Light")?;

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
            client.join(gid, cid).await?;
            println!("Joined");
        }
        AudioEngineSubcommands::Leave {
            guild_id,
            channel_id,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            client.leave(gid, cid).await?;
            println!("Left");
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
            client
                .play(
                    gid,
                    cid,
                    QueueName::from(queue),
                    TapId(tap),
                    AudioRequestString::from(config.resolve_alias(&request)),
                    Volume::from(volume),
                    DiscordUserId::from(String::new()),
                )
                .await?;
            println!("Playing");
        }
        AudioEngineSubcommands::SetVolume {
            guild_id,
            channel_id,
            track_id,
            volume,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            client
                .set_volume(gid, cid, TrackId::from(track_id), Volume::from(volume))
                .await?;
            println!("Volume set");
        }
        AudioEngineSubcommands::Stop {
            guild_id,
            channel_id,
            track_id,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let tid = track_id.parse::<u64>().context("Invalid track ID")?;
            client.stop(gid, cid, TrackId::from(tid)).await?;
            println!("Stopped");
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
                    anyhow::bail!("Invalid filter type. Options: all, music, tts");
                }
            };
            client.stop_many(gid, cid, filter_type).await?;
            println!("Stopped");
        }
        AudioEngineSubcommands::NextMusic {
            guild_id,
            channel_id,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            client.next_music(gid, cid).await?;
            println!("Next music");
        }
        AudioEngineSubcommands::Pause {
            guild_id,
            channel_id,
            queue,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            client.pause(gid, cid, QueueName::from(queue)).await?;
            println!("Paused");
        }
        AudioEngineSubcommands::Resume {
            guild_id,
            channel_id,
            queue,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            client.resume(gid, cid, QueueName::from(queue)).await?;
            println!("Resumed");
        }
        AudioEngineSubcommands::GetSessionState {
            guild_id,
            channel_id,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let state = client.get_session_state(gid, cid).await?;
            formatter::print_session_state_native(state);
        }
        AudioEngineSubcommands::GetSessionsInGuild { guild_id } => {
            let gid = resolve_guild_id(guild_id)?;
            let sessions = client.get_sessions_in_guild(gid).await?;
            formatter::print_sessions_list(sessions);
        }
    }

    Ok(())
}
