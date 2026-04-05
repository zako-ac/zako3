use anyhow::{Context, Result};
use http::{HeaderName, HeaderValue};
use jsonrpsee::http_client::HttpClientBuilder;
use zako3_audio_engine_controller::AudioEngineRpcClient;
use zako3_types::{
    AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName, TapName, TrackId, UserId,
    Volume, hq::DiscordUserId,
};

use crate::config::Config;
use crate::services::audio_engine::cli::{AudioEngineCommands, AudioEngineSubcommands};
use crate::services::audio_engine::formatter;

pub async fn handle_command(ae_addr: String, cmd: AudioEngineCommands) -> Result<()> {
    let config = Config::load().unwrap_or_default();
    let ae_token = config
        .get_active_context()
        .and_then(|ctx| ctx.ae_token.clone());
    let client = connect(ae_addr, ae_token).await?;

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
        AudioEngineSubcommands::Leave { guild_id } => {
            let gid = resolve_guild_id(guild_id)?;
            let success = client.leave(gid).await?;
            println!("Leave Success: {}", success);
        }
        AudioEngineSubcommands::Play {
            guild_id,
            queue,
            tap,
            request,
            volume,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let track_id = client
                .play(
                    gid,
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
            track_id,
            volume,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let success = client
                .set_volume(gid, TrackId::from(track_id), Volume::from(volume))
                .await?;
            println!("SetVolume Success: {}", success);
        }
        AudioEngineSubcommands::Stop { guild_id, track_id } => {
            let gid = resolve_guild_id(guild_id)?;
            let tid = track_id.parse::<u64>().context("Invalid track ID")?;
            let success = client.stop(gid, TrackId::from(tid)).await?;
            println!("Stop Success: {}", success);
        }
        AudioEngineSubcommands::StopMany {
            guild_id,
            filter,
            user_id,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let filter_type = match filter.to_lowercase().as_str() {
                "all" => AudioStopFilter::All,
                "music" => AudioStopFilter::Music,
                "tts" => {
                    let uid = user_id.context("user_id is required for tts filter")?;
                    AudioStopFilter::TTS(UserId::from(uid))
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Invalid filter type. Options: all, music, tts"
                    ));
                }
            };

            let success = client.stop_many(gid, filter_type).await?;
            println!("StopMany Success: {}", success);
        }
        AudioEngineSubcommands::NextMusic { guild_id } => {
            let gid = resolve_guild_id(guild_id)?;
            let success = client.next_music(gid).await?;
            println!("NextMusic Success: {}", success);
        }
        AudioEngineSubcommands::GetSessionState { guild_id } => {
            let gid = resolve_guild_id(guild_id)?;
            let state = client.get_session_state(gid).await?;
            formatter::print_session_state_native(state);
        }
    }

    Ok(())
}

async fn connect(
    addr: String,
    token: Option<String>,
) -> Result<jsonrpsee::http_client::HttpClient> {
    let endpoint = if addr.starts_with("http") {
        addr
    } else {
        format!("http://{}", addr)
    };

    println!("Connecting to Audio Engine at {}...", endpoint);

    let mut builder = HttpClientBuilder::default();
    if let Some(token) = token {
        let mut headers = jsonrpsee::http_client::HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-ae-token"),
            HeaderValue::from_str(&token).context("Invalid token format")?,
        );
        builder = builder.set_headers(headers);
    }

    builder
        .build(endpoint)
        .context("Failed to connect to Audio Engine service")
}
