use anyhow::{Context, Result};
use tonic::transport::Channel;
use zako3_audio_engine_protos::audio_engine_client::AudioEngineClient;
use zako3_audio_engine_protos::{
    AudioStopFilter, GetSessionStateRequest, JoinRequest, LeaveRequest, NextMusicRequest,
    PlayRequest, SetVolumeRequest, StopManyRequest, StopRequest, audio_stop_filter,
};

use crate::config::Config;
use crate::services::audio_engine::cli::{AudioEngineCommands, AudioEngineSubcommands};
use crate::services::audio_engine::formatter;

pub async fn handle_command(ae_addr: String, cmd: AudioEngineCommands) -> Result<()> {
    let mut client = connect(ae_addr).await?;
    let config = Config::load().unwrap_or_default();

    // Helper closure to resolve guild_id from option or context
    let resolve_guild_id = |gid: Option<String>| -> Result<u64> {
        if let Some(id) = gid {
            Ok(config.resolve_alias(&id).parse()?)
        } else if let Some(ctx) = config.get_active_context() {
            if let Some(ref default_id) = ctx.default_guild_id {
                Ok(config.resolve_alias(default_id).parse()?)
            } else {
                anyhow::bail!("Guild ID not provided and no default found in current context")
            }
        } else {
            anyhow::bail!("Guild ID not provided and no active context found")
        }
    };

    match cmd.command {
        AudioEngineSubcommands::Join {
            guild_id,
            channel_id,
        } => {
            let request = tonic::Request::new(JoinRequest {
                guild_id: resolve_guild_id(guild_id)?,
                channel_id: config.resolve_alias(&channel_id).parse()?,
            });
            let response = client.join(request).await?;
            formatter::print_ok("Join Response:", response.into_inner());
        }
        AudioEngineSubcommands::Leave { guild_id } => {
            let request = tonic::Request::new(LeaveRequest {
                guild_id: resolve_guild_id(guild_id)?,
            });
            let response = client.leave(request).await?;
            formatter::print_ok("Leave Response:", response.into_inner());
        }
        AudioEngineSubcommands::Play {
            guild_id,
            queue,
            tap,
            request,
            volume,
        } => {
            let request = tonic::Request::new(PlayRequest {
                guild_id: resolve_guild_id(guild_id)?,
                queue_name: queue,
                tap_name: tap,
                audio_request_string: config.resolve_alias(&request),
                volume,
            });
            let response = client.play(request).await?;
            formatter::print_play(response.into_inner());
        }
        AudioEngineSubcommands::SetVolume {
            guild_id,
            track_id,
            volume,
        } => {
            let request = tonic::Request::new(SetVolumeRequest {
                guild_id: resolve_guild_id(guild_id)?,
                track_id,
                volume,
            });
            let response = client.set_volume(request).await?;
            formatter::print_ok("SetVolume Response:", response.into_inner());
        }
        AudioEngineSubcommands::Stop { guild_id, track_id } => {
            let request = tonic::Request::new(StopRequest {
                guild_id: resolve_guild_id(guild_id)?,
                track_id,
            });
            let response = client.stop(request).await?;
            formatter::print_ok("Stop Response:", response.into_inner());
        }
        AudioEngineSubcommands::StopMany {
            guild_id,
            filter,
            user_id,
        } => {
            let filter_type = match filter.to_lowercase().as_str() {
                "all" => Some(audio_stop_filter::FilterType::All(
                    audio_stop_filter::All {},
                )),
                "music" => Some(audio_stop_filter::FilterType::Music(
                    audio_stop_filter::Music {},
                )),
                "tts" => {
                    let uid = user_id.context("user_id is required for tts filter")?;
                    Some(audio_stop_filter::FilterType::Tts(audio_stop_filter::Tts {
                        user_id: uid,
                    }))
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Invalid filter type. Options: all, music, tts"
                    ));
                }
            };

            let request = tonic::Request::new(StopManyRequest {
                guild_id: resolve_guild_id(guild_id)?,
                filter: Some(AudioStopFilter { filter_type }),
            });
            let response = client.stop_many(request).await?;
            formatter::print_ok("StopMany Response:", response.into_inner());
        }
        AudioEngineSubcommands::NextMusic { guild_id } => {
            let request = tonic::Request::new(NextMusicRequest {
                guild_id: resolve_guild_id(guild_id)?,
            });
            let response = client.next_music(request).await?;
            formatter::print_ok("NextMusic Response:", response.into_inner());
        }
        AudioEngineSubcommands::GetSessionState { guild_id } => {
            let request = tonic::Request::new(GetSessionStateRequest {
                guild_id: resolve_guild_id(guild_id)?,
            });
            let response = client.get_session_state(request).await?;
            formatter::print_session_state(response.into_inner());
        }
    }

    Ok(())
}

async fn connect(addr: String) -> Result<AudioEngineClient<Channel>> {
    let endpoint = if addr.starts_with("http") {
        addr
    } else {
        format!("http://{}", addr)
    };

    println!("Connecting to Audio Engine at {}...", endpoint);
    AudioEngineClient::connect(endpoint)
        .await
        .context("Failed to connect to Audio Engine service")
}
