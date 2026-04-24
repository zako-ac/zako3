use crate::{ui, util, util::VoiceStateExt, Context, Error};
use hq_core::{service::UserVoiceInfo, CoreError};
use hq_types::{
    hq::settings::{PartialUserSettings, UserSettingsField},
    hq::{DiscordUserId, TapId, TapName},
    AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName, UserId, Volume,
};
use poise::serenity_prelude as serenity;

#[derive(Debug, poise::ChoiceParameter)]
pub enum TtsTarget {
    #[name = "Myself"]
    Myself,
    #[name = "Everyone"]
    All,
}

#[derive(Debug, poise::ChoiceParameter)]
pub enum VoiceScope {
    #[name = "This Guild"]
    GuildUser,
    #[name = "All Guilds"]
    User,
}

#[poise::command(
    slash_command,
    subcommands("speak", "stop", "skip", "voice"),
    name_localized("ko", "tts"),
    description_localized("en-US", "Text-to-speech commands"),
    description_localized("ko", "텍스트 음성 변환 명령")
)]
pub async fn tts(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Queue a TTS message.
#[poise::command(
    slash_command,
    name_localized("ko", "말하기"),
    description_localized("en-US", "Queue a TTS message"),
    description_localized("ko", "TTS 메시지 대기열에 추가")
)]
pub async fn speak(
    ctx: Context<'_>,
    #[description = "The message to speak"]
    #[description_localized("ko", "음성으로 변환할 메시지")]
    message: String,
    #[description = "TTS voice to use (defaults to your current voice setting)"]
    #[description_localized("ko", "사용할 TTS 음성 (기본값: 현재 설정된 음성)")]
    voice: Option<String>,
) -> Result<(), Error> {
    // Extract non-Send guild data before the first await.
    let (guild_id, message_channel_id, user_voice_info) = {
        let guild = ctx
            .guild()
            .ok_or_else(|| Error::Other("This command must be used in a server.".to_string()))?;
        let info: Option<UserVoiceInfo> = guild
            .voice_states
            .get(&ctx.author().id)
            .map(|vs| vs.to_user_voice_info());
        (
            GuildId::from(guild.id.get()),
            ChannelId::from(ctx.channel_id().get()),
            info,
        )
    };

    let discord_id = ctx.author().id.get().to_string();
    let service = &ctx.data().service;

    let user_id_optional = service
        .tap
        .get_user_by_discord_id(&discord_id)
        .await?
        .map(|u| u.id.clone());

    let settings = service
        .user_settings
        .get_effective_settings(&user_id_optional, Some(&guild_id.to_string()))
        .await?;

    // Resolve target voice channels using the same routing logic as auto-TTS.
    let channel_ids = service
        .playback
        .resolve_tts_channels(guild_id, message_channel_id, user_voice_info, &settings)
        .await?;

    if channel_ids.is_empty() {
        ctx.send(
            poise::CreateReply::default()
                .content("You need to be in a voice channel (or I need to be in one).")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    // Resolve tap ID: prefer the `voice` argument, then user settings, then fallback.
    let tap_id = resolve_tap_id(service, voice.as_deref(), &settings.tts_voice).await?;

    let audio_request = AudioRequestString::from(message.clone());
    let discord_user_id = DiscordUserId::from(discord_id.clone());
    let queue_name = QueueName::from(format!("tts_{discord_id}"));

    for channel_id in channel_ids {
        service
            .audio_engine
            .play(
                guild_id,
                channel_id,
                queue_name.clone(),
                tap_id.clone(),
                audio_request.clone(),
                Volume::from(1.0f32),
                discord_user_id.clone(),
            )
            .await?;
    }

    let preview = if message.len() > 50 {
        format!("{}…", &message[..50])
    } else {
        message.clone()
    };
    ctx.say(ui::messages::tts_queued(&preview)).await?;
    Ok(())
}

async fn resolve_tap_id(
    service: &hq_core::Service,
    voice_arg: Option<&str>,
    settings_voice: &Option<TapId>,
) -> Result<TapId, Error> {
    const FALLBACK: &str = "google";

    if let Some(name) = voice_arg {
        if let Some(tap) = service.tap.get_tap_by_name(&TapName::from(name.to_string())).await? {
            return Ok(tap.id);
        }
        return Err(hq_core::CoreError::NotFound(format!("Voice '{}' not found.", name)).into());
    }

    if let Some(tap_id) = settings_voice {
        return Ok(tap_id.clone());
    }

    // Fallback: resolve default tap by name
    if let Some(tap) = service.tap.get_tap_by_name(&TapName::from(FALLBACK.to_string())).await? {
        return Ok(tap.id);
    }

    Err(hq_core::CoreError::NotFound(format!("Default tap '{}' not found.", FALLBACK)).into())
}

/// Stop TTS playback.
#[poise::command(
    slash_command,
    name_localized("ko", "정지"),
    description_localized("en-US", "Stop TTS playback"),
    description_localized("ko", "TTS 재생 정지")
)]
pub async fn stop(
    ctx: Context<'_>,
    #[description = "Who to stop: yourself (default) or everyone (requires Mute Members)"]
    #[description_localized("ko", "정지 대상: 본인 (기본값) 또는 전체 (음소거 멤버 권한 필요)")]
    target: Option<TtsTarget>,
    #[description = "Stop a specific user's TTS (requires Mute Members)"]
    #[description_localized("ko", "특정 사용자의 TTS 정지 (음소거 멤버 권한 필요)")]
    user: Option<serenity::User>,
) -> Result<(), Error> {
    if user.is_some() || matches!(target, Some(TtsTarget::All)) {
        require_mute_members(ctx).await?;
    }

    let service = &ctx.data().service;
    let session = util::resolve_session(ctx, None).await?;
    let self_discord_id = ctx.author().id.get().to_string();

    match (&target, &user) {
        (_, Some(u)) => {
            let uid = u.id.get().to_string();
            service
                .audio_engine
                .stop_many(
                    session.guild_id,
                    session.channel_id,
                    AudioStopFilter::TTS(UserId::from(uid)),
                )
                .await?;
        }
        (Some(TtsTarget::All), _) => {
            let state = service
                .audio_engine
                .get_session_state(session.guild_id, session.channel_id)
                .await?;
            let tts_users: Vec<String> = state
                .queues
                .keys()
                .filter_map(|q| q.to_string().strip_prefix("tts_").map(|s| s.to_string()))
                .collect();
            for uid in tts_users {
                service
                    .audio_engine
                    .stop_many(
                        session.guild_id,
                        session.channel_id,
                        AudioStopFilter::TTS(UserId::from(uid)),
                    )
                    .await?;
            }
        }
        _ => {
            service
                .audio_engine
                .stop_many(
                    session.guild_id,
                    session.channel_id,
                    AudioStopFilter::TTS(UserId::from(self_discord_id)),
                )
                .await?;
        }
    }

    ctx.say(ui::messages::tts_stopped()).await?;
    Ok(())
}

/// Skip the current TTS message.
#[poise::command(
    slash_command,
    name_localized("ko", "건너뛰기"),
    description_localized("en-US", "Skip the current TTS message"),
    description_localized("ko", "현재 TTS 메시지 건너뛰기")
)]
pub async fn skip(
    ctx: Context<'_>,
    #[description = "Who to skip: yourself (default) or everyone (requires Mute Members)"]
    #[description_localized("ko", "건너뛸 대상: 본인 (기본값) 또는 전체 (음소거 멤버 권한 필요)")]
    target: Option<TtsTarget>,
    #[description = "Skip a specific user's TTS message (requires Mute Members)"]
    #[description_localized("ko", "특정 사용자의 TTS 건너뛰기 (음소거 멤버 권한 필요)")]
    user: Option<serenity::User>,
) -> Result<(), Error> {
    if user.is_some() || matches!(target, Some(TtsTarget::All)) {
        require_mute_members(ctx).await?;
    }

    let service = &ctx.data().service;
    let session = util::resolve_session(ctx, None).await?;
    let self_discord_id = ctx.author().id.get().to_string();
    let state = service
        .audio_engine
        .get_session_state(session.guild_id, session.channel_id)
        .await?;

    let target_queues: Vec<String> = match (&target, &user) {
        (_, Some(u)) => vec![format!("tts_{}", u.id.get())],
        (Some(TtsTarget::All), _) => state
            .queues
            .keys()
            .filter(|q| q.to_string().starts_with("tts_"))
            .map(|q| q.to_string())
            .collect(),
        _ => vec![format!("tts_{self_discord_id}")],
    };

    for queue_name_str in target_queues {
        let qn = QueueName::from(queue_name_str);
        if let Some(first_track_id) = state
            .queues
            .get(&qn)
            .and_then(|q| q.first())
            .map(|t| t.track_id)
        {
            service
                .audio_engine
                .stop(session.guild_id, session.channel_id, first_track_id)
                .await?;
        }
    }

    ctx.say(ui::messages::tts_skipped()).await?;
    Ok(())
}

/// Change your TTS voice.
#[poise::command(
    slash_command,
    name_localized("ko", "음성"),
    description_localized("en-US", "Change your TTS voice"),
    description_localized("ko", "TTS 음성 변경")
)]
pub async fn voice(
    ctx: Context<'_>,
    #[description = "The voice provider to use"]
    #[description_localized("ko", "사용할 음성 제공자")]
    provider: Option<String>,
    #[description = "Where to save this preference (default: All Guilds)"]
    #[description_localized("ko", "설정을 저장할 범위 (기본값: 모든 서버)")]
    scope: Option<VoiceScope>,
) -> Result<(), Error> {
    let service = &ctx.data().service;
    let discord_id = ctx.author().id.to_string();
    let username = &ctx.author().name;

    let user = service
        .auth
        .get_or_create_user(&discord_id, username, None, None, None)
        .await?;

    if let Some(ref tap_name) = provider {
        // Validate the tap exists and is accessible
        let taps = service.tap.list_by_user(user.id.clone()).await?;
        let found = taps
            .data
            .iter()
            .find(|t| t.tap.name.eq_ignore_ascii_case(tap_name));

        if found.is_none() {
            return Err(CoreError::NotFound(format!("Voice '{}' not found.", tap_name)).into());
        }

        let found = found.unwrap();
        let tap_id = TapId::from(found.tap.id.clone());
        let partial = PartialUserSettings {
            tts_voice: UserSettingsField::Normal(Some(tap_id)),
            ..PartialUserSettings::empty()
        };

        match scope.unwrap_or(VoiceScope::User) {
            VoiceScope::GuildUser => {
                let guild_id_str = ctx
                    .guild_id()
                    .ok_or_else(|| {
                        Error::Other(
                            "This command must be used in a server for guild scope.".to_string(),
                        )
                    })?
                    .get()
                    .to_string();
                service
                    .user_settings
                    .save_guild_user_settings(&user.id, &guild_id_str, partial)
                    .await?;
            }
            VoiceScope::User => {
                service
                    .user_settings
                    .save_settings(user.id.clone(), partial)
                    .await?;
            }
        }

        ctx.say(ui::messages::voice_changed(tap_name)).await?;
    } else {
        // Show available voices
        let taps = service.tap.list_by_user(user.id).await?;
        let embed = ui::embeds::tap_list_embed(&taps.data);
        ctx.send(
            poise::CreateReply::default()
                .content("Choose a voice from your Taps:")
                .embed(embed),
        )
        .await?;
    }

    Ok(())
}

async fn require_mute_members(ctx: Context<'_>) -> Result<(), Error> {
    let has_perm = ctx
        .author_member()
        .await
        .and_then(|m| ctx.guild().map(|g| g.member_permissions(&m).mute_members()))
        .unwrap_or(false);

    if !has_perm {
        return Err(Error::Forbidden);
    }
    Ok(())
}
