use tokio::io::AsyncRead;

use crate::{
    error::ZakoResult,
    types::{ChannelId, GuildId},
};

pub trait DiscordService: Send + Sync + 'static {
    async fn join_voice_channel(&self, guild_id: GuildId, channel_id: ChannelId) -> ZakoResult<()>;
    async fn leave_voice_channel(&self, guild_id: GuildId) -> ZakoResult<()>;
    async fn play_audio(&self, guild_id: GuildId, stream: impl AsyncRead) -> ZakoResult<()>;
}
