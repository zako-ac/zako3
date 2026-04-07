use crate::{ui, Context, Error};
use hq_types::{ChannelId, GuildId};
use poise::serenity_prelude as serenity;

#[poise::command(
    slash_command,
    subcommands("enable", "disable", "list"),
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
    let channel = resolve_channel(ctx, channel).await?;
    let guild_id = GuildId::from(channel.guild_id.get());
    let channel_id = ChannelId::from(channel.id.get());

    ctx.data()
        .service
        .tts_channel
        .set_enabled(&guild_id, &channel_id, true)
        .await?;

    ctx.say(ui::messages::channel_enabled(channel.id)).await?;
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
    let channel = resolve_channel(ctx, channel).await?;
    let guild_id = GuildId::from(channel.guild_id.get());
    let channel_id = ChannelId::from(channel.id.get());

    ctx.data()
        .service
        .tts_channel
        .set_enabled(&guild_id, &channel_id, false)
        .await?;

    ctx.say(ui::messages::channel_disabled(channel.id)).await?;
    Ok(())
}

#[poise::command(
    slash_command,
    name_localized("ko", "목록"),
    description_localized("en-US", "List all TTS-enabled channels"),
    description_localized("ko", "TTS 활성화된 채널 목록"),
    required_permissions = "MANAGE_CHANNELS"
)]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| Error::Other("This command must be used in a server.".to_string()))?;

    let channel_ids = ctx
        .data()
        .service
        .tts_channel
        .get_enabled_channels(&GuildId::from(guild_id.get()))
        .await?;

    let embed = ui::embeds::channel_list_embed(&channel_ids);

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

async fn resolve_channel(
    ctx: Context<'_>,
    channel: Option<serenity::GuildChannel>,
) -> Result<serenity::GuildChannel, Error> {
    if let Some(c) = channel {
        return Ok(c);
    }
    ctx.channel_id()
        .to_channel(&ctx)
        .await?
        .guild()
        .ok_or_else(|| Error::Other("This command must be used in a server channel.".to_string()))
}
