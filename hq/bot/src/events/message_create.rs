use std::sync::Arc;

use hq_core::{CoreResult, Service};
use hq_types::{
    hq::{DiscordUserId, Tap, TapName, TextReadingRule, UserSettings},
    AudioRequestString, ChannelId, GuildId, QueueName,
};
use serenity::{
    all::{Context, EventHandler, VoiceState},
    async_trait,
};
use tracing::instrument;

pub struct MessageCreateHandler {
    pub service: Arc<Service>,
}

const FALLBACK_TAP_NAME: &str = "google";

fn fallback_tap_name() -> TapName {
    TapName::from(FALLBACK_TAP_NAME.to_string())
}

#[async_trait]
impl EventHandler for MessageCreateHandler {
    #[instrument(name = "handle_message_create", skip(self, ctx, msg), fields(guild_id = ?msg.guild_id, channel_id = ?msg.channel_id, author_id = ?msg.author.id))]
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
        None => return Ok(()), // Ignore DMs
    };

    let voice_state = msg
        .guild(&ctx.cache)
        .and_then(|guild| guild.voice_states.get(&msg.author.id).cloned());

    let message_channel_id = ChannelId::from(msg.channel_id.get());
    let author_id = DiscordUserId::from(msg.author.id.get().to_string());

    if service.tts_channel.is_enabled(&message_channel_id).await? {
        let content = msg.content.trim();

        let (settings, tap_name) = {
            let user = service
                .tap
                .get_user_by_discord_id(&author_id.to_string())
                .await?;

            if let Some(hq_user) = user {
                let settings = service
                    .user_settings
                    .get_effective_settings(&hq_user.id, Some(&guild_id.to_string()))
                    .await?;

                let tap_name = resolve_tap_name_for_user(&service, &settings).await?;

                (settings, tap_name)
            } else {
                (UserSettings::default(), fallback_tap_name())
            }
        };

        let channel_ids = resolve_channels(
            &service,
            guild_id,
            message_channel_id,
            voice_state,
            &settings,
        )
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
                            tap_name.clone(),
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

async fn resolve_channels(
    service: &Service,
    guild_id: GuildId,
    message_channel_id: ChannelId,
    user_voice_state: Option<VoiceState>,
    settings: &UserSettings,
) -> CoreResult<Vec<ChannelId>> {
    let bot_channel_ids = service
        .audio_engine
        .get_sessions_in_guild(guild_id)
        .await?
        .into_iter()
        .map(|s| s.channel_id)
        .collect::<Vec<_>>();

    if bot_channel_ids.contains(&message_channel_id) {
        // Voice channel message channel
        Ok(vec![message_channel_id])
    } else {
        match settings.text_reading_rule {
            TextReadingRule::Always => {
                if let Some(user_voice_state) = user_voice_state {
                    // User is in a voice channel, use that channel
                    let user_channel_id = user_voice_state
                        .channel_id
                        .map(|cid| ChannelId::from(cid.get()))
                        .ok_or_else(|| {
                            hq_core::CoreError::Internal(
                                "User voice state missing channel_id".to_string(),
                            )
                        })?;

                    Ok(vec![user_channel_id])
                } else {
                    // broadcast
                    Ok(bot_channel_ids)
                }
            }
            TextReadingRule::InVoiceChannel | TextReadingRule::OnMicMute => {
                if let Some(user_voice_state) = user_voice_state {
                    let user_channel_id = user_voice_state
                        .channel_id
                        .map(|cid| ChannelId::from(cid.get()))
                        .ok_or_else(|| {
                            hq_core::CoreError::Internal(
                                "User voice state missing channel_id".to_string(),
                            )
                        })?;

                    match settings.text_reading_rule {
                        TextReadingRule::InVoiceChannel => Ok(vec![user_channel_id]),
                        TextReadingRule::OnMicMute => {
                            let mic_muted = user_voice_state.mute || user_voice_state.self_mute;

                            if mic_muted {
                                Ok(vec![user_channel_id])
                            } else {
                                Ok(vec![])
                            }
                        }
                        _ => Ok(vec![]), // Unreachable due to outer match
                    }
                } else {
                    Ok(vec![]) // User not in voice channel
                }
            }
        }
    }
}

fn queue_name(user_id: &DiscordUserId, queue_tts: bool) -> QueueName {
    if queue_tts {
        format!("tts-{}", user_id.to_string()).into()
    } else {
        format!("temp-{}-{}", user_id.to_string(), uuid::Uuid::new_v4()).into()
    }
}

async fn resolve_tap_name_for_user(
    service: &Service,
    settings: &hq_types::hq::UserSettings,
) -> CoreResult<TapName> {
    let tap_name = match &settings.tts_voice {
        Some(tap_id) => {
            let tap: Option<Tap> = service.tap.get_tap_internal(tap_id.clone()).await?;

            if let Some(t) = tap {
                t.name
            } else {
                fallback_tap_name()
            }
        }
        None => fallback_tap_name(),
    };

    Ok(tap_name)
}
