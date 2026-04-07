use crate::{Context, Error};
use hq_core::service::UserVoiceInfo;
use hq_types::{ChannelId, GuildId, SessionState};
use poise::serenity_prelude as serenity;
use serenity::model::permissions::Permissions;

/// Extension trait for converting serenity voice state types to [`UserVoiceInfo`].
///
/// Defined here (in hq-bot) because orphan rules prevent implementing foreign
/// traits for foreign types in hq-core.
pub trait VoiceStateExt {
    fn to_user_voice_info(&self) -> UserVoiceInfo;
}

impl VoiceStateExt for serenity::VoiceState {
    fn to_user_voice_info(&self) -> UserVoiceInfo {
        UserVoiceInfo {
            channel_id: self.channel_id.map(|cid| ChannelId::from(cid.get())),
            mute: self.mute,
            self_mute: self.self_mute,
        }
    }
}

/// Returns the bot's first active session in this guild.
pub async fn get_bot_session(ctx: Context<'_>) -> Result<SessionState, Error> {
    let guild_id = require_guild_id(ctx)?;
    let sessions = ctx
        .data()
        .service
        .audio_engine
        .get_sessions_in_guild(guild_id)
        .await?;
    sessions.into_iter().next().ok_or(Error::BotNotInVoiceChannel)
}

/// Returns the invoking user's current voice channel ID from the serenity guild cache.
pub fn get_user_voice_channel(ctx: Context<'_>) -> Result<(serenity::ChannelId, ChannelId), Error> {
    let guild = ctx
        .guild()
        .ok_or_else(|| Error::Other("This command must be used in a server.".to_string()))?;
    let serenity_cid = guild
        .voice_states
        .get(&ctx.author().id)
        .and_then(|vs| vs.channel_id)
        .ok_or(Error::NotInVoiceChannel)?;
    Ok((serenity_cid, ChannelId::from(serenity_cid.get())))
}

/// Returns the guild ID from context, or an error if used outside a guild.
pub fn require_guild_id(ctx: Context<'_>) -> Result<GuildId, Error> {
    ctx.guild_id()
        .map(|id| GuildId::from(id.get()))
        .ok_or_else(|| Error::Other("This command must be used in a server.".to_string()))
}

/// Returns `Ok(true)` if the user has CONNECT + SPEAK permissions in the given voice channel.
pub async fn user_can_access_voice_channel(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
    channel_id: serenity::ChannelId,
    user_id: serenity::UserId,
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
