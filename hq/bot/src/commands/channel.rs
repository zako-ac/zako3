use crate::{Context, Error};
use hq_types::{ChannelId, GuildId};
use poise::serenity_prelude as serenity;

#[poise::command(
    slash_command,
    subcommands("enable", "disable"),
    name_localized("ko", "채널"),
    description_localized("ko", "TTS 채널 관리")
)]
pub async fn channel(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(
    slash_command,
    name_localized("ko", "활성화"),
    description_localized("en-US", "Enable TTS in a channel"),
    description_localized("ko", "채널에서 TTS 활성화"),
    required_permissions = "MANAGE_CHANNELS"
)]
pub async fn enable(
    ctx: Context<'_>,
    #[description = "The channel to enable TTS in"]
    #[description_localized("ko", "TTS를 활성화할 채널")]
    channel: Option<serenity::GuildChannel>,
) -> Result<(), Error> {
    let channel = if let Some(c) = channel {
        c
    } else {
        ctx.channel_id()
            .to_channel(&ctx)
            .await?
            .guild()
            .ok_or_else(|| "Could not get current channel".to_string())?
    };

    let guild_id = GuildId::from(channel.guild_id.get());
    let channel_id = ChannelId::from(channel.id.get());

    ctx.data()
        .service
        .tts_channel
        .set_enabled(&guild_id, &channel_id, true)
        .await?;

    ctx.say(format!("TTS enabled in <#{}>", channel.id)).await?;

    Ok(())
}

#[poise::command(
    slash_command,
    name_localized("ko", "비활성화"),
    description_localized("en-US", "Disable TTS in a channel"),
    description_localized("ko", "채널에서 TTS 비활성화"),
    required_permissions = "MANAGE_CHANNELS"
)]
pub async fn disable(
    ctx: Context<'_>,
    #[description = "The channel to disable TTS in"]
    #[description_localized("ko", "TTS를 비활성화할 채널")]
    channel: Option<serenity::GuildChannel>,
) -> Result<(), Error> {
    let channel = if let Some(c) = channel {
        c
    } else {
        ctx.channel_id()
            .to_channel(&ctx)
            .await?
            .guild()
            .ok_or_else(|| "Could not get current channel".to_string())?
    };

    let guild_id = GuildId::from(channel.guild_id.get());
    let channel_id = ChannelId::from(channel.id.get());

    ctx.data()
        .service
        .tts_channel
        .set_enabled(&guild_id, &channel_id, false)
        .await?;

    ctx.say(format!("TTS disabled in <#{}>", channel.id))
        .await?;

    Ok(())
}
