use crate::{
    error::ZakoResult,
    service::{DiscordService, StateService},
    types::{ChannelId, GuildId, SessionState},
};

pub struct Controller<DS, SS>
where
    DS: DiscordService,
    SS: StateService,
{
    discord_service: DS,
    state_service: SS,
}

impl<DS, SS> Controller<DS, SS>
where
    DS: DiscordService,
    SS: StateService,
{
    pub async fn join(&self, guild_id: GuildId, channel_id: ChannelId) -> ZakoResult<()> {
        self.discord_service
            .join_voice_channel(guild_id, channel_id)
            .await?;

        let session = SessionState {
            guild_id,
            channel_id: channel_id,
            queues: Default::default(),
        };

        self.state_service.save_session(&session).await?;

        Ok(())
    }

    pub async fn leave(&self, guild_id: GuildId) -> ZakoResult<()> {
        self.discord_service.leave_voice_channel(guild_id).await?;
        self.state_service.delete_session(guild_id).await?;
        Ok(())
    }
}
