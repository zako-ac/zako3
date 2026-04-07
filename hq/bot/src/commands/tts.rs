use crate::{ui, util::VoiceStateExt, Context, Error};
use hq_core::{service::UserVoiceInfo, CoreError};
use hq_types::{
    hq::{DiscordUserId, TapId},
    AudioRequestString, ChannelId, GuildId, QueueName, TapName, Volume,
};
use poise::serenity_prelude as serenity;

#[derive(Debug, poise::ChoiceParameter)]
pub enum TtsTarget {
    #[name = "Myself"]
    Myself,
    #[name = "Everyone"]
    All,
}

#[poise::command(
    slash_command,
    subcommands("speak", "stop", "skip"),
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
    let username = ctx.author().name.clone();
    let service = &ctx.data().service;

    // Resolve the user's account and effective settings.
    let user = service
        .auth
        .get_or_create_user(&discord_id, &username, None, None)
        .await?;

    let settings = service
        .user_settings
        .get_effective_settings(&user.id, Some(&guild_id.to_string()))
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

    // Resolve tap name: prefer the `voice` argument, then user settings, then fallback.
    let tap_name = resolve_tap_name(service, voice.as_deref(), &settings.tts_voice).await?;

    let audio_request = AudioRequestString::from(message.clone());
    let discord_user_id = DiscordUserId::from(discord_id.clone());
    let queue_name = QueueName::from(format!("tts-{discord_id}"));

    for channel_id in channel_ids {
        service
            .audio_engine
            .play(
                guild_id,
                channel_id,
                queue_name.clone(),
                tap_name.clone(),
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

async fn resolve_tap_name(
    service: &hq_core::Service,
    voice_arg: Option<&str>,
    settings_voice: &Option<TapId>,
) -> Result<TapName, Error> {
    const FALLBACK: &str = "google";

    if let Some(name) = voice_arg {
        return Ok(TapName::from(name.to_string()));
    }

    if let Some(tap_id) = settings_voice {
        if let Some(tap) = service.tap.get_tap_internal(tap_id.clone()).await? {
            return Ok(tap.name);
        }
    }

    Ok(TapName::from(FALLBACK.to_string()))
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

    // TODO: call audio_engine.tts_stop(guild_id, target_user_id)

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

    // TODO: call audio_engine.tts_skip(guild_id, target_user_id)

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
) -> Result<(), Error> {
    let service = &ctx.data().service;
    let discord_id = ctx.author().id.to_string();
    let username = &ctx.author().name;

    let user = service
        .auth
        .get_or_create_user(&discord_id, username, None, None)
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

        // TODO: persist voice preference via user_settings

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
        .and_then(|m| {
            ctx.guild()
                .map(|g| g.member_permissions(&*m).mute_members())
        })
        .unwrap_or(false);

    if !has_perm {
        return Err(Error::Forbidden);
    }
    Ok(())
}
