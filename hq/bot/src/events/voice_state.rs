use poise::serenity_prelude as serenity;
use serenity::{Context, EventHandler, async_trait, model::voice::VoiceState};
use zako3_states::VoiceStateService;

pub struct VoiceStateHandler {
    pub voice_state_service: VoiceStateService,
}

#[async_trait]
impl EventHandler for VoiceStateHandler {
    async fn voice_state_update(&self, _ctx: Context, _old: Option<VoiceState>, new: VoiceState) {
        let discord_user_id = new.user_id.to_string();

        let guild_id = match new.guild_id {
            Some(g) => g.get(),
            None => return,
        };

        match new.channel_id {
            Some(ch) => {
                let _ = self
                    .voice_state_service
                    .set_user_channel(&discord_user_id, guild_id, ch.get())
                    .await;
            }
            None => {
                let _ = self
                    .voice_state_service
                    .remove_user_from_guild(&discord_user_id, guild_id)
                    .await;
            }
        }
    }
}
