use std::collections::HashMap;

use ae_protocol::{
    AudioEngineCommand, AudioEngineCommandRequest, AudioEngineCommandResponse,
    AudioEngineRpcClient, AudioEngineSessionCommand, AudioPlayRequest, SessionInfo,
};
use anyhow::{Context, Result, bail};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use zako3_types::{
    AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName, SessionState, TrackId,
    Volume,
    hq::{DiscordUserId, TapId},
};

use crate::config::Config;
use crate::services::audio_engine::cli::{AudioEngineCommands, AudioEngineSubcommands};
use crate::services::audio_engine::formatter;

/// Talks straight to one audio engine.
///
/// The engine's `execute` endpoint is the whole audio command vocabulary, so a
/// developer can drive a single engine directly — which is the useful thing in
/// development anyway. Anything that needs to be routed across engines (which
/// bot joins which channel) belongs to HQ, not here.
pub async fn handle_command(ae_addr: String, cmd: AudioEngineCommands) -> Result<()> {
    let config = Config::load().unwrap_or_default();

    let endpoint = if !ae_addr.is_empty() {
        ae_addr
    } else if let Some(ctx) = config.get_active_context() {
        ctx.ae_addr.clone()
    } else {
        "http://127.0.0.1:8090".to_string()
    };

    println!("Connecting to audio engine at {}...", endpoint);
    let client = HttpClientBuilder::default()
        .build(&endpoint)
        .context("Failed to build audio engine client")?;

    let resolve_guild_id = |gid: Option<String>| -> Result<GuildId> {
        let id: u64 = if let Some(id) = gid {
            config.resolve_alias(&id).parse()?
        } else if let Some(ctx) = config.get_active_context() {
            if let Some(ref default_id) = ctx.default_guild_id {
                config.resolve_alias(default_id).parse()?
            } else {
                bail!("Guild ID not provided and no default found in current context")
            }
        } else {
            bail!("Guild ID not provided and no active context found")
        };
        Ok(GuildId::from(id))
    };

    let session_of = |gid: GuildId, cid: ChannelId| SessionInfo {
        guild_id: gid,
        channel_id: cid,
    };

    match cmd.command {
        AudioEngineSubcommands::Join {
            guild_id,
            channel_id,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let resp = send(
                &client,
                Some(session_of(gid, cid)),
                AudioEngineCommand::Join,
            )
            .await?;
            expect_ok(resp)?;
            println!("Joined");
        }
        AudioEngineSubcommands::Leave {
            guild_id,
            channel_id,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let resp = send(
                &client,
                Some(session_of(gid, cid)),
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Leave),
            )
            .await?;
            expect_ok(resp)?;
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
            let resp = send(
                &client,
                Some(session_of(gid, cid)),
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Play(
                    AudioPlayRequest {
                        queue_name: QueueName::from(queue),
                        tap_id: TapId(tap),
                        ars: AudioRequestString::from(config.resolve_alias(&request)),
                        volume: Volume::from(volume),
                        initiator: DiscordUserId::from(String::new()),
                        headers: HashMap::new(),
                    },
                )),
            )
            .await?;
            expect_ok(resp)?;
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
            let resp = send(
                &client,
                Some(session_of(gid, cid)),
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::SetVolume {
                    track_id: TrackId::from(track_id),
                    volume: Volume::from(volume),
                }),
            )
            .await?;
            expect_ok(resp)?;
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
            let resp = send(
                &client,
                Some(session_of(gid, cid)),
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Stop(TrackId::from(
                    tid,
                ))),
            )
            .await?;
            expect_ok(resp)?;
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
                    AudioStopFilter::TTS(zako3_types::UserId::from(uid.to_string()))
                }
                _ => {
                    bail!("Invalid filter type. Options: all, music, tts");
                }
            };
            let resp = send(
                &client,
                Some(session_of(gid, cid)),
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::StopMany(
                    filter_type,
                )),
            )
            .await?;
            expect_ok(resp)?;
            println!("Stopped");
        }
        AudioEngineSubcommands::NextMusic {
            guild_id,
            channel_id,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let resp = send(
                &client,
                Some(session_of(gid, cid)),
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::NextMusic),
            )
            .await?;
            expect_ok(resp)?;
            println!("Next music");
        }
        AudioEngineSubcommands::Pause {
            guild_id,
            channel_id,
            queue,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let resp = send(
                &client,
                Some(session_of(gid, cid)),
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Pause(
                    QueueName::from(queue),
                )),
            )
            .await?;
            expect_ok(resp)?;
            println!("Paused");
        }
        AudioEngineSubcommands::Resume {
            guild_id,
            channel_id,
            queue,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let resp = send(
                &client,
                Some(session_of(gid, cid)),
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Resume(
                    QueueName::from(queue),
                )),
            )
            .await?;
            expect_ok(resp)?;
            println!("Resumed");
        }
        AudioEngineSubcommands::GetSessionState {
            guild_id,
            channel_id,
        } => {
            let gid = resolve_guild_id(guild_id)?;
            let cid = ChannelId::from(config.resolve_alias(&channel_id).parse::<u64>()?);
            let resp = send(
                &client,
                Some(session_of(gid, cid)),
                AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::GetSessionState),
            )
            .await?;
            match resp {
                AudioEngineCommandResponse::SessionState(state) => {
                    formatter::print_session_state_native(state);
                }
                other => bail!("unexpected response: {other:?}"),
            }
        }
        AudioEngineSubcommands::GetSessionsInGuild { guild_id } => {
            // Which sessions exist is a guild-wide question, and this client
            // only talks to one engine. So ask the engine for its own live
            // voice connections and report those: an engine addresses exactly
            // the channels its bots are physically in, which is the same truth
            // HQ reads from Discord.
            let gid = resolve_guild_id(guild_id)?;
            let resp = send(&client, None, AudioEngineCommand::FetchDiscordVoiceState).await?;
            let live = match resp {
                AudioEngineCommandResponse::DiscordVoiceState(sessions) => sessions,
                other => bail!("unexpected response: {other:?}"),
            };

            let mut states: Vec<SessionState> = Vec::new();
            for info in live.into_iter().filter(|s| s.guild_id == gid) {
                let state = match send(
                    &client,
                    Some(info),
                    AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::GetSessionState),
                )
                .await
                {
                    Ok(AudioEngineCommandResponse::SessionState(state)) => state,
                    _ => SessionState {
                        guild_id: info.guild_id,
                        channel_id: info.channel_id,
                        queues: Default::default(),
                    },
                };
                states.push(state);
            }
            formatter::print_sessions_list(states);
        }
    }

    Ok(())
}

async fn send(
    client: &HttpClient,
    session: Option<SessionInfo>,
    command: AudioEngineCommand,
) -> Result<AudioEngineCommandResponse> {
    let request = AudioEngineCommandRequest {
        session,
        command,
        headers: HashMap::new(),
        idempotency_key: None,
    };
    AudioEngineRpcClient::execute(client, request)
        .await
        .context("audio engine RPC failed")
}

fn expect_ok(response: AudioEngineCommandResponse) -> Result<()> {
    match response {
        AudioEngineCommandResponse::Ok => Ok(()),
        AudioEngineCommandResponse::Error(e) => bail!("audio engine error: {e:?}"),
        other => bail!("unexpected response: {other:?}"),
    }
}
