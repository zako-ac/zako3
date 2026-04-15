use std::time::Duration;

use crate::{Context, Error, ui, util};
use hq_core::{CoreResult, Service};
use hq_types::{
    AudioRequestString, ChannelId, GuildId, QueueName,
    hq::{DiscordUserId, TapName},
};
use poise::serenity_prelude as serenity;

/// Join a voice channel and play a bot-join announcement.
pub(crate) async fn bot_join_and_announce(
    service: &Service,
    serenity_ctx: &serenity::Context,
    guild_id: GuildId,
    _serenity_guild_id: serenity::GuildId,
    channel_id: ChannelId,
) -> CoreResult<()> {
    let bot_user_id = serenity_ctx.cache.current_user().id;

    service.audio_engine.join(guild_id, channel_id).await?;

    let queue_name: QueueName = format!("temp-alert-{}", uuid::Uuid::new_v4()).into();

    let service_clone = service.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let _ = service_clone
            .audio_engine
            .play(
                guild_id,
                channel_id,
                queue_name,
                TapName::from("papago".to_string()),
                AudioRequestString::from(format!("자코 등장")),
                1.0.into(),
                DiscordUserId::from(bot_user_id.get().to_string()),
            )
            .await;
    });

    Ok(())
}

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
    #[channel_types("Voice")]
    channel: Option<serenity::GuildChannel>,
) -> Result<(), Error> {
    // Extract guild/channel data before any await.
    let (serenity_guild_id, guild_id, channel_id) = {
        match channel {
            Some(c) => (
                c.guild_id,
                GuildId::from(c.guild_id.get()),
                ChannelId::from(c.id.get()),
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
                (
                    guild.id,
                    GuildId::from(guild.id.get()),
                    ChannelId::from(serenity_cid.get()),
                )
            }
        }
    };

    let service = &ctx.data().service;
    bot_join_and_announce(
        service,
        ctx.serenity_context(),
        guild_id,
        serenity_guild_id,
        channel_id,
    )
    .await?;
    service
        .intended_vc
        .add(u64::from(guild_id), u64::from(channel_id))
        .await?;

    ctx.say(ui::messages::bot_joined(u64::from(channel_id).into()))
        .await?;
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
    #[channel_types("Voice")]
    channel: Option<serenity::GuildChannel>,
) -> Result<(), Error> {
    let session = util::resolve_session(ctx, channel).await?;
    let service = &ctx.data().service;

    // Remove from intended_vc before leaving so a racing voice_state_update won't auto-rejoin.
    service
        .intended_vc
        .remove(u64::from(session.guild_id), u64::from(session.channel_id))
        .await?;
    service
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
    #[channel_types("Voice")]
    channel: serenity::GuildChannel,
) -> Result<(), Error> {
    let session = util::resolve_session(ctx, None).await?;
    let new_serenity_guild_id = channel.guild_id;
    let new_channel_id = ChannelId::from(channel.id.get());

    let service = &ctx.data().service;

    // Remove old channel from intended_vc, leave it, then join+announce new channel.
    service
        .intended_vc
        .remove(u64::from(session.guild_id), u64::from(session.channel_id))
        .await?;
    service
        .audio_engine
        .leave(session.guild_id, session.channel_id)
        .await?;
    bot_join_and_announce(
        service,
        ctx.serenity_context(),
        session.guild_id,
        new_serenity_guild_id,
        new_channel_id,
    )
    .await?;
    service
        .intended_vc
        .add(u64::from(session.guild_id), u64::from(new_channel_id))
        .await?;

    ctx.say(ui::messages::bot_moved(u64::from(new_channel_id).into()))
        .await?;
    Ok(())
}
