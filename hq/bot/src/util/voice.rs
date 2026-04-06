use poise::serenity_prelude as serenity;
use serenity::{ChannelId, GuildId, UserId, model::permissions::Permissions};

/// Returns `Ok(true)` if the user has CONNECT + SPEAK permissions in the given voice channel,
/// `Ok(false)` if they do not, or `Err` if the guild/channel is not in the serenity cache.
pub async fn user_can_access_voice_channel(
    ctx: &serenity::Context,
    guild_id: GuildId,
    channel_id: ChannelId,
    user_id: UserId,
) -> Result<bool, serenity::Error> {
    let guild = guild_id
        .to_guild_cached(&ctx.cache)
        .ok_or(serenity::Error::Other("guild not in cache"))?;

    let member = guild.member(ctx, user_id).await?;

    let channel = guild
        .channels
        .get(&channel_id)
        .ok_or(serenity::Error::Other("channel not in cache"))?;

    let permissions = guild.user_permissions_in(channel, &member);

    Ok(permissions.contains(Permissions::CONNECT | Permissions::SPEAK))
}
