use std::hash::{BuildHasher, Hasher, RandomState};

use crate::{Context, Error, ui, util};
use hq_core::CoreError;
use hq_types::{
    AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName, Volume,
    hq::{DiscordUserId, TapId, TapName},
};
use poise::serenity_prelude as serenity;

const MUSIC_QUEUE: &str = "music";

#[derive(Debug, poise::ChoiceParameter)]
pub enum StopScope {
    #[name = "현재 트랙"]
    Current,
    #[name = "전체 대기열 (정지 및 초기화)"]
    Queue,
}

/// Resolves the voice channel to play in, auto-joining the bot if it isn't already there.
async fn resolve_play_channel(
    ctx: Context<'_>,
    channel: Option<serenity::GuildChannel>,
) -> Result<(GuildId, ChannelId), Error> {
    let (guild_id, serenity_guild_id, user_serenity_cid) = {
        let guild = ctx
            .guild()
            .ok_or_else(|| Error::Other("이 명령어는 서버에서만 사용할 수 있어요.".to_string()))?;
        let user_cid = guild
            .voice_states
            .get(&ctx.author().id)
            .and_then(|vs| vs.channel_id);
        (GuildId::from(guild.id.get()), guild.id, user_cid)
    };

    let channel_id = if let Some(ch) = channel {
        ChannelId::from(ch.id.get())
    } else {
        let user_cid = user_serenity_cid
            .map(|cid| ChannelId::from(cid.get()))
            .ok_or(Error::NotInVoiceChannel)?;
        let sessions = ctx
            .data()
            .service
            .audio_engine
            .get_sessions_in_guild(guild_id)
            .await?;
        if !sessions.iter().any(|s| s.channel_id == user_cid) {
            crate::commands::voice::join_channel(
                &ctx.data().service,
                ctx.serenity_context(),
                guild_id,
                serenity_guild_id,
                user_cid,
            )
            .await?;
        }
        user_cid
    };

    Ok((guild_id, channel_id))
}

/// Search and play audio.
#[poise::command(
    slash_command,
    name_localized("ko", "재생"),
    description_localized("en-US", "Search and play audio"),
    description_localized("ko", "음악 검색 및 재생")
)]
pub async fn play(
    ctx: Context<'_>,
    #[description = "Search query or URL"]
    #[description_localized("ko", "검색어 또는 URL")]
    query: String,
    #[description = "Tap name to use as audio source (default: youtube)"]
    #[description_localized("ko", "음악 소스로 사용할 Tap 이름 (기본값: youtube)")]
    source: Option<String>,
    #[description = "Voice channel to play in (default: your current channel)"]
    #[description_localized("ko", "재생할 음성 채널 (기본값: 현재 채널)")]
    #[channel_types("Voice")]
    channel: Option<serenity::GuildChannel>,
) -> Result<(), Error> {
    let (guild_id, channel_id) = resolve_play_channel(ctx, channel).await?;

    let source_name = source.unwrap_or_else(|| "youtube".to_string());
    let tap_id = resolve_tap_id(ctx, &source_name).await?;
    let queue_name = QueueName::from(MUSIC_QUEUE.to_string());
    let audio_request = AudioRequestString::from(query.clone());
    let discord_user_id = DiscordUserId::from(ctx.author().id.get().to_string());

    // Send "Loading…" immediately so the user sees a response while the engine works.
    let reply_handle = ctx
        .send(poise::CreateReply::default().content("로딩 중…"))
        .await?;

    // Run the engine call and state fetch. Any error edits the existing reply rather
    // than letting it propagate to on_error (which would leave "Loading…" stuck).
    let result = do_play(
        ctx,
        guild_id,
        channel_id,
        queue_name,
        tap_id,
        audio_request,
        discord_user_id,
        &query,
    )
    .await;

    match result {
        Ok(reply) => reply_handle.edit(ctx, reply).await?,
        Err(ref e) => {
            if e.is_internal() {
                tracing::error!("play command error: {e:?}");
            }
            let embed = ui::embeds::error_embed(e.to_user_message().as_ref());
            reply_handle
                .edit(ctx, poise::CreateReply::default().content("").embed(embed))
                .await?;
        }
    }

    Ok(())
}

async fn resolve_tap_id(ctx: Context<'_>, name: &str) -> Result<TapId, Error> {
    let service = &ctx.data().service;
    if let Some(tap) = service
        .tap
        .get_tap_by_name(&TapName::from(name.to_string()))
        .await?
    {
        Ok(tap.id)
    } else {
        Err(CoreError::NotFound(format!("Tap '{}' not found.", name)).into())
    }
}

#[allow(clippy::too_many_arguments)]
async fn do_play<'a>(
    ctx: Context<'a>,
    guild_id: GuildId,
    channel_id: ChannelId,
    queue_name: QueueName,
    tap_id: TapId,
    audio_request: AudioRequestString,
    discord_user_id: hq_types::hq::DiscordUserId,
    query: &str,
) -> Result<poise::CreateReply, Error> {
    let ae = &ctx.data().service.audio_engine;

    ae.play(
        guild_id,
        channel_id,
        queue_name.clone(),
        tap_id,
        audio_request,
        Volume::from(1.0f32),
        discord_user_id,
    )
    .await?;

    // The engine confirms the track synchronously, so session state is current.
    let state = ae.get_session_state(guild_id, channel_id).await?;
    let music_q = QueueName::from(MUSIC_QUEUE.to_string());

    if let Some(tracks) = state.queues.get(&music_q) {
        let pos = tracks.len();
        if pos > 0 {
            let embed = ui::embeds::track_queued_embed(&tracks[pos - 1], pos);
            return Ok(poise::CreateReply::default().content("").embed(embed));
        }
    }

    Ok(poise::CreateReply::default().content(format!("대기열에 추가했어요: *{query}*")))
}

const CONGRATS: &[&str] = &[
    "신부, 신부, 입장!",
    "좋은사랑하세요",
    "이쁜사랑하세요",
    "저랑 결혼하시게요...?",
    "쟤네 둘이 이제서야 결혼하네",
];

const CONGRATS_TWISTED: &[&str] = &["쟤네 둘이 이제서야 파혼하네"];

/// Play the wedding song (easter egg).
#[poise::command(
    slash_command,
    name_localized("ko", "결혼"),
    description_localized("en-US", "Play the wedding song"),
    description_localized("ko", "결혼 축하 음악 재생")
)]
pub async fn wedding(
    ctx: Context<'_>,
    #[description = "Voice channel to play in (default: your current channel)"]
    #[description_localized("ko", "재생할 음성 채널 (기본값: 현재 채널)")]
    #[channel_types("Voice")]
    channel: Option<serenity::GuildChannel>,
    #[description = "Play twisted wedding?"]
    #[description_localized("ko", "뒤틀린 결혼행진곡")]
    twisted: Option<bool>,
) -> Result<(), Error> {
    let (guild_id, channel_id) = resolve_play_channel(ctx, channel).await?;
    let tap_id = resolve_tap_id(ctx, "wedding").await?;
    let queue_name = QueueName::from(MUSIC_QUEUE.to_string());
    let discord_user_id = DiscordUserId::from(ctx.author().id.get().to_string());

    let twisted = twisted.unwrap_or_else(|| RandomState::new().build_hasher().finish() % 4 == 0);
    let ars = if twisted { "c" } else { "wedding" };
    ctx.data()
        .service
        .audio_engine
        .play(
            guild_id,
            channel_id,
            queue_name,
            tap_id,
            AudioRequestString::from(ars.to_string()),
            Volume::from(1.0f32),
            discord_user_id,
        )
        .await?;

    let idx = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0)
        % CONGRATS.len();
    let msg = CONGRATS[idx];
    ctx.say(msg).await?;
    Ok(())
}

/// Stop playback.
#[poise::command(
    slash_command,
    name_localized("ko", "정지"),
    description_localized("en-US", "Stop playback"),
    description_localized("ko", "재생 정지")
)]
pub async fn stop(
    ctx: Context<'_>,
    #[description = "What to stop: current track (default) or entire queue"]
    #[description_localized("ko", "정지할 범위: 현재 트랙 (기본값) 또는 전체 대기열")]
    scope: Option<StopScope>,
    #[description = "Voice channel to use (defaults to your current channel)"]
    #[description_localized("ko", "사용할 음성 채널 (기본값: 현재 채널)")]
    #[channel_types("Voice")]
    channel: Option<serenity::GuildChannel>,
) -> Result<(), Error> {
    let session = util::resolve_session(ctx, channel).await?;
    let ae = &ctx.data().service.audio_engine;

    match scope.unwrap_or(StopScope::Current) {
        StopScope::Current => {
            let state = ae
                .get_session_state(session.guild_id, session.channel_id)
                .await?;
            let music_q = QueueName::from(MUSIC_QUEUE.to_string());
            let track = state
                .queues
                .get(&music_q)
                .and_then(|q| q.first())
                .ok_or(Error::NothingPlaying)?;
            ae.stop(session.guild_id, session.channel_id, track.track_id)
                .await?;
        }
        StopScope::Queue => {
            ae.stop_many(session.guild_id, session.channel_id, AudioStopFilter::Music)
                .await?;
        }
    }

    ctx.say(ui::messages::playback_stopped()).await?;
    Ok(())
}

/// Skip one or more tracks.
#[poise::command(
    slash_command,
    name_localized("ko", "건너뛰기"),
    description_localized("en-US", "Skip the current track or a number of tracks"),
    description_localized("ko", "현재 트랙 또는 여러 트랙 건너뛰기")
)]
pub async fn skip(
    ctx: Context<'_>,
    #[description = "Number of tracks to skip (default: 1)"]
    #[description_localized("ko", "건너뛸 트랙 수 (기본값: 1)")]
    count: Option<u32>,
    #[description = "Voice channel to use (defaults to your current channel)"]
    #[description_localized("ko", "사용할 음성 채널 (기본값: 현재 채널)")]
    #[channel_types("Voice")]
    channel: Option<serenity::GuildChannel>,
) -> Result<(), Error> {
    ctx.defer().await?; // In case skipping takes a moment, especially for multiple tracks.

    let count = count.unwrap_or(1).max(1);
    let session = util::resolve_session(ctx, channel).await?;
    let actor_id = ctx.author().id.get().to_string();

    for _ in 0..count {
        ctx.data()
            .service
            .playback
            .skip_music(
                session.guild_id.into(),
                session.channel_id.into(),
                &actor_id,
            )
            .await?;
    }

    ctx.say(ui::messages::skipped(count)).await?;
    Ok(())
}

/// Adjust the playback volume (0–150).
#[poise::command(
    slash_command,
    name_localized("ko", "볼륨"),
    description_localized("en-US", "Adjust the playback volume (0–150)"),
    description_localized("ko", "재생 볼륨 조절 (0–150)")
)]
pub async fn volume(
    ctx: Context<'_>,
    #[description = "Volume level from 0 to 150"]
    #[description_localized("ko", "볼륨 수준 (0에서 150까지)")]
    level: u8,
    #[description = "Voice channel to use (defaults to your current channel)"]
    #[description_localized("ko", "사용할 음성 채널 (기본값: 현재 채널)")]
    #[channel_types("Voice")]
    channel: Option<serenity::GuildChannel>,
) -> Result<(), Error> {
    if level > 150 {
        return Err(Error::InvalidVolume);
    }

    let session = util::resolve_session(ctx, channel).await?;
    let ae = &ctx.data().service.audio_engine;

    let state = ae
        .get_session_state(session.guild_id, session.channel_id)
        .await?;
    let music_q = QueueName::from(MUSIC_QUEUE.to_string());
    let track = state
        .queues
        .get(&music_q)
        .and_then(|q| q.first())
        .ok_or(Error::NothingPlaying)?;

    ae.set_volume(
        session.guild_id,
        session.channel_id,
        track.track_id,
        Volume::from(level as f32 / 100.0),
    )
    .await?;

    ctx.say(ui::messages::volume_set(level)).await?;
    Ok(())
}
