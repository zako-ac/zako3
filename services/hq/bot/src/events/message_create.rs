use std::sync::Arc;

use hq_core::{CoreResult, Service};
use hq_types::{
    hq::{DiscordUserId, TapId},
    AudioRequestString, ChannelId, GuildId, QueueName,
};
use serenity::{
    all::{Context, EventHandler},
    async_trait,
};
use tracing::instrument;

use crate::util::VoiceStateExt;

pub struct MessageCreateHandler {
    pub service: Arc<Service>,
}

const FALLBACK_TAP_NAME: &str = "google";

#[async_trait]
impl EventHandler for MessageCreateHandler {
    #[instrument(
        name = "message_create", skip(self, ctx, msg),
        fields(
            guild_id = ?msg.guild_id.map(u64::from),
            channel_id = %msg.channel_id.to_string(),
            author_id = %msg.author.id.to_string(), 
            content = %msg.content,
            message_id = %msg.id.to_string(),
        )
    )]
    async fn message(&self, ctx: Context, msg: serenity::all::Message) {
        tracing::info!("Received message: {}", msg.content);

        if let Err(e) = handle_message_create(self.service.clone(), ctx, msg).await {
            tracing::warn!("Error handling message create: {}", e);
        }
    }
}

async fn handle_message_create(
    service: Arc<Service>,
    ctx: Context,
    msg: serenity::all::Message,
) -> CoreResult<()> {
    let guild_id = match msg.guild_id {
        Some(gid) => GuildId::from(gid.get()),
        None => return Ok(()),
    };

    if msg.author.bot {
        return Ok(());
    }

    let user_voice_info = msg.guild(&ctx.cache).and_then(|guild| {
        guild
            .voice_states
            .get(&msg.author.id)
            .map(|vs| vs.to_user_voice_info())
    });

    let message_channel_id = ChannelId::from(msg.channel_id.get());
    let author_id = DiscordUserId::from(msg.author.id.get().to_string());

    if service.tts_channel.is_enabled(&message_channel_id).await? {
        let content = msg.content.trim();

            let user = service
                .tap
                .get_user_by_discord_id(&author_id.to_string())
                .await?;

            let user_id_optional = user.as_ref().map(|u| u.id.clone());

            let settings = service
                    .user_settings
                    .get_effective_settings(&user_id_optional, Some(&guild_id.to_string()))
                    .await?;

        let tap_id = resolve_tap_id_for_user(&service, &settings).await?;

        let channel_ids = service
            .playback
            .resolve_tts_channels(guild_id, message_channel_id, user_voice_info, &settings)
            .await?;

        if !channel_ids.is_empty() {
            let mapped: AudioRequestString = service
                .mapping
                .map_text(
                    content.to_string(),
                    guild_id,
                    message_channel_id,
                    author_id.clone(),
                    settings.text_mappings,
                    settings.emoji_mappings,
                )
                .await?
                .trim()
                .to_string()
                .into();

            let queue_name = queue_name(&author_id, settings.enable_tts_queue);

            if !mapped.to_string().is_empty() {
                for channel_id in channel_ids {
                    service
                        .audio_engine
                        .play(
                            guild_id,
                            channel_id,
                            queue_name.clone(),
                            tap_id.clone(),
                            mapped.clone(),
                            1.0.into(),
                            author_id.clone(),
                        )
                        .await?;
                }
            }
        }
    }

    Ok(())
}

fn queue_name(user_id: &DiscordUserId, queue_tts: bool) -> QueueName {
    if queue_tts {
        format!("tts_{}", user_id).into()
    } else {
        format!("temp-{}-{}", user_id, uuid::Uuid::new_v4()).into()
    }
}

async fn resolve_tap_id_for_user(
    service: &Service,
    settings: &hq_types::hq::UserSettings,
) -> CoreResult<TapId> {
    if let Some(tap_id) = &settings.tts_voice {
        return Ok(tap_id.clone());
    }
    // Fallback: resolve default tap by name
    if let Some(tap) = service.tap.get_tap_by_name(&hq_types::hq::TapName::from(FALLBACK_TAP_NAME.to_string())).await? {
        return Ok(tap.id);
    }
    Err(hq_core::CoreError::NotFound(format!("Default tap '{}' not found.", FALLBACK_TAP_NAME)))
}
