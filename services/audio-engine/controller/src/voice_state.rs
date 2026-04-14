use std::sync::{Arc, OnceLock};

use serenity::{async_trait, model::voice::VoiceState, prelude::*};

use zako3_audio_engine_core::{
    engine::session_manager::SessionManager,
    types::{ChannelId, GuildId},
};

pub struct VoiceStateHandler {
    pub session_manager: Arc<OnceLock<Arc<SessionManager>>>,
}

#[async_trait]
impl EventHandler for VoiceStateHandler {
    async fn voice_state_update(
        &self,
        ctx: Context,
        old: Option<VoiceState>,
        new: VoiceState,
    ) {
        let session_manager = match self.session_manager.get() {
            Some(sm) => sm,
            None => {
                tracing::warn!("VoiceStateHandler: session_manager not yet initialized");
                return;
            }
        };

        // Only care about the bot's own voice state changes
        let bot_user_id = match ctx.http.get_current_user().await {
            Ok(u) => u.id,
            Err(e) => {
                tracing::warn!("Failed to get current user: {e}");
                return;
            }
        };
        if new.user_id != bot_user_id {
            return;
        }

        let guild_id = match new.guild_id {
            Some(g) => GuildId::from(g.get()),
            None => return,
        };

        // Bot is still connected somewhere — not a disconnect event
        if new.channel_id.is_some() {
            return;
        }

        // Retrieve the channel the bot was kicked from
        let channel_id = match old.as_ref().and_then(|o| o.channel_id) {
            Some(ch) => ChannelId::from(ch.get()),
            None => return,
        };

        // If no in-memory session exists, our own leave() already cleaned up
        if session_manager.as_ref().get_session(guild_id, channel_id).is_none() {
            return;
        }

        tracing::warn!(
            guild_id = %guild_id,
            channel_id = %channel_id,
            "Bot disconnected externally; cleaning up session"
        );

        if let Err(e) = session_manager
            .as_ref()
            .cleanup_session(guild_id, channel_id)
            .await
        {
            tracing::error!(
                guild_id = %guild_id,
                channel_id = %channel_id,
                "Failed to clean up session after external disconnect: {e}"
            );
        }
    }
}
