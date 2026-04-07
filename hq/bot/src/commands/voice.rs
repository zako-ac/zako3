use crate::{ui, util, Context, Error};
use hq_types::{ChannelId, GuildId};
use poise::serenity_prelude as serenity;

/// Join a voice channel.
#[poise::command(
    slash_command,
    name_localized("ko", "참가"),
    description_localized("en-US", "Join a voice channel"),
    description_localized("ko", "음성 채널에 참가")
)]
pub async fn join(
    ctx: Context<'_>,
    #[description = "The voice channel to join (defaults to your current channel)"]
    #[description_localized("ko", "참가할 음성 채널 (기본값: 현재 채널)")]
    channel: Option<serenity::GuildChannel>,
) -> Result<(), Error> {
    // Extract all guild data before the first await.
    let (guild_id, channel_id, channel_name) = {
        match channel {
            Some(c) => (
                GuildId::from(c.guild_id.get()),
                ChannelId::from(c.id.get()),
                c.name.clone(),
            ),
            None => {
                let guild = ctx.guild().ok_or_else(|| {
                    Error::Other("This command must be used in a server.".to_string())
                })?;
                let serenity_cid = guild
                    .voice_states
                    .get(&ctx.author().id)
                    .and_then(|vs| vs.channel_id)
                    .ok_or(Error::NotInVoiceChannel)?;
                let name = guild
                    .channels
                    .get(&serenity_cid)
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| serenity_cid.to_string());
                (
                    GuildId::from(guild.id.get()),
                    ChannelId::from(serenity_cid.get()),
                    name,
                )
            }
        }
    };

    ctx.data()
        .service
        .audio_engine
        .join(guild_id, channel_id)
        .await?;

    ctx.say(ui::messages::bot_joined(&channel_name)).await?;
    Ok(())
}

/// Leave the current voice channel.
#[poise::command(
    slash_command,
    name_localized("ko", "나가기"),
    description_localized("en-US", "Leave the voice channel"),
    description_localized("ko", "음성 채널에서 나가기")
)]
pub async fn leave(
    ctx: Context<'_>,
    #[description = "Voice channel to leave (defaults to your current channel)"]
    #[description_localized("ko", "나갈 음성 채널 (기본값: 현재 채널)")]
    channel: Option<serenity::GuildChannel>,
) -> Result<(), Error> {
    let session = util::resolve_session(ctx, channel).await?;

    ctx.data()
        .service
        .audio_engine
        .leave(session.guild_id, session.channel_id)
        .await?;

    ctx.say(ui::messages::bot_left()).await?;
    Ok(())
}

/// Move the bot to a different voice channel.
#[poise::command(
    slash_command,
    name_localized("ko", "이동"),
    description_localized("en-US", "Move the bot to a different voice channel"),
    description_localized("ko", "봇을 다른 음성 채널로 이동")
)]
pub async fn move_to(
    ctx: Context<'_>,
    #[description = "The voice channel to move to"]
    #[description_localized("ko", "이동할 음성 채널")]
    channel: serenity::GuildChannel,
) -> Result<(), Error> {
    let session = util::resolve_session(ctx, None).await?;
    let new_channel_id = ChannelId::from(channel.id.get());
    let channel_name = channel.name.clone();

    let ae = &ctx.data().service.audio_engine;
    ae.leave(session.guild_id, session.channel_id).await?;
    ae.join(session.guild_id, new_channel_id).await?;

    ctx.say(ui::messages::bot_moved(&channel_name)).await?;
    Ok(())
}
