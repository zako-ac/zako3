use crate::{ui, util, Context, Error};
use poise::serenity_prelude as serenity;
use hq_types::{AudioStopFilter, QueueName, Track, UserId};

const MUSIC_QUEUE_PREFIX: &str = "music";
const TTS_QUEUE_PREFIX: &str = "tts_";

#[derive(Debug, poise::ChoiceParameter)]
pub enum ClearTarget {
    #[name = "Music queue"]
    Music,
    #[name = "My TTS queue"]
    Tts,
    #[name = "All queues"]
    All,
}

#[poise::command(
    slash_command,
    subcommands("music", "tts", "web"),
    name_localized("ko", "대기열"),
    description_localized("en-US", "View the current queue"),
    description_localized("ko", "현재 대기열 보기")
)]
pub async fn queue(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Show the current music queue.
#[poise::command(
    slash_command,
    name_localized("ko", "음악"),
    description_localized("en-US", "Show the music queue"),
    description_localized("ko", "음악 대기열 표시")
)]
pub async fn music(
    ctx: Context<'_>,
    #[description = "Voice channel to use (defaults to your current channel)"]
    #[description_localized("ko", "사용할 음성 채널 (기본값: 현재 채널)")]
    channel: Option<serenity::GuildChannel>,
) -> Result<(), Error> {
    let session = util::resolve_session(ctx, channel).await?;
    let state = ctx
        .data()
        .service
        .audio_engine
        .get_session_state(session.guild_id, session.channel_id)
        .await?;

    let music_q = QueueName::from(MUSIC_QUEUE_PREFIX.to_string());
    let tracks: &[Track] = state.queues.get(&music_q).map(Vec::as_slice).unwrap_or(&[]);

    let embed = ui::embeds::queue_music_embed(tracks);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Show the upcoming TTS messages.
#[poise::command(
    slash_command,
    name_localized("ko", "tts"),
    description_localized("en-US", "Show the TTS queue"),
    description_localized("ko", "TTS 대기열 표시")
)]
pub async fn tts(
    ctx: Context<'_>,
    #[description = "Voice channel to use (defaults to your current channel)"]
    #[description_localized("ko", "사용할 음성 채널 (기본값: 현재 채널)")]
    channel: Option<serenity::GuildChannel>,
) -> Result<(), Error> {
    let session = util::resolve_session(ctx, channel).await?;
    let state = ctx
        .data()
        .service
        .audio_engine
        .get_session_state(session.guild_id, session.channel_id)
        .await?;

    let tts_tracks: Vec<&Track> = state
        .queues
        .iter()
        .filter(|(name, _)| name.to_string().starts_with(TTS_QUEUE_PREFIX))
        .flat_map(|(_, tracks)| tracks.iter())
        .collect();

    let embed = ui::embeds::queue_tts_embed(&tts_tracks);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

/// Open the web-based queue interface.
#[poise::command(
    slash_command,
    name_localized("ko", "열기"),
    description_localized("en-US", "Get a link to the web queue interface"),
    description_localized("ko", "웹 대기열 인터페이스 링크")
)]
pub async fn web(ctx: Context<'_>) -> Result<(), Error> {
    let service = &ctx.data().service;
    let url = ui::messages::queue_web_url(&service.config.zako_website_url);
    let login_url = service.auth.get_login_url(Some("/queue"));

    let embed = ui::embeds::web_link_embed("Web Queue", "View and manage the queue in your browser.");
    let open_button = serenity::CreateButton::new_link(&url).label("Open");
    let login_button = serenity::CreateButton::new_link(&login_url).label("Login");
    let row = serenity::CreateActionRow::Buttons(vec![open_button, login_button]);
    ctx.send(
        poise::CreateReply::default()
            .embed(embed)
            .components(vec![row]),
    )
    .await?;
    Ok(())
}

/// Clear a queue.
#[poise::command(
    slash_command,
    name_localized("ko", "클리어"),
    description_localized("en-US", "Clear the music or TTS queue"),
    description_localized("ko", "음악 또는 TTS 대기열 지우기")
)]
pub async fn clear(
    ctx: Context<'_>,
    #[description = "Which queue to clear (default: music)"]
    #[description_localized("ko", "지울 대기열 (기본값: 음악)")]
    target: Option<ClearTarget>,
    #[description = "Voice channel to use (defaults to your current channel)"]
    #[description_localized("ko", "사용할 음성 채널 (기본값: 현재 채널)")]
    channel: Option<serenity::GuildChannel>,
) -> Result<(), Error> {
    let target = target.unwrap_or(ClearTarget::Music);
    let session = util::resolve_session(ctx, channel).await?;

    let filter = match target {
        ClearTarget::Music => AudioStopFilter::Music,
        ClearTarget::Tts => {
            let discord_id = ctx.author().id.get().to_string();
            AudioStopFilter::TTS(UserId::from(discord_id))
        }
        ClearTarget::All => AudioStopFilter::All,
    };

    let label = match filter {
        AudioStopFilter::Music => "music",
        AudioStopFilter::TTS(_) => "TTS",
        AudioStopFilter::All => "all",
    };

    ctx.data()
        .service
        .audio_engine
        .stop_many(session.guild_id, session.channel_id, filter)
        .await?;

    ctx.say(ui::messages::cleared(label)).await?;
    Ok(())
}
