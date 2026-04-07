use std::sync::Arc;

use hq_core::{CoreResult, Service};
use hq_types::{
    AudioRequestString, ChannelId, GuildId, QueueName,
    hq::{DiscordUserId, Tap, TapName, UserJoinLeaveAlert, UserSettings},
};
use poise::serenity_prelude as serenity;
use serenity::{Context, EventHandler, async_trait, model::voice::VoiceState};
use zako3_states::VoiceStateService;

pub struct VoiceStateHandler {
    pub voice_state_service: VoiceStateService,
    pub service: Arc<Service>,
}

#[async_trait]
impl EventHandler for VoiceStateHandler {
    async fn cache_ready(&self, ctx: Context, guilds: Vec<serenity::GuildId>) {
        for guild_id in guilds {
            let voice_states: Vec<_> = {
                let guild = ctx.cache.guild(guild_id);
                match guild {
                    None => continue,
                    Some(g) => g
                        .voice_states
                        .iter()
                        .filter_map(|(user_id, vs)| {
                            let channel_id = vs.channel_id?;
                            let channel_name = g
                                .channels
                                .get(&channel_id)
                                .map(|c| c.name.clone())
                                .unwrap_or_else(|| channel_id.get().to_string());
                            Some((
                                user_id.to_string(),
                                guild_id.get(),
                                channel_id.get(),
                                g.name.clone(),
                                channel_name,
                            ))
                        })
                        .collect(),
                }
            };

            for (discord_user_id, gid, cid, guild_name, channel_name) in voice_states {
                let _ = self
                    .voice_state_service
                    .set_user_channel(&discord_user_id, gid, cid, guild_name, channel_name)
                    .await;
            }
        }
    }

    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        let discord_user_id = new.user_id.to_string();

        let guild_id_typed = match new.guild_id {
            Some(g) => g,
            None => return,
        };
        let guild_id = guild_id_typed.get();

        // Update voice state tracking (existing logic)
        match new.channel_id {
            Some(ch) => {
                // Extract names while holding cache refs, then drop before .await
                let (guild_name, channel_name) = {
                    let guild = ctx.cache.guild(guild_id_typed);
                    let gn = guild
                        .as_ref()
                        .map(|g| g.name.clone())
                        .unwrap_or_else(|| guild_id.to_string());
                    let cn = guild
                        .as_ref()
                        .and_then(|g| g.channels.get(&ch))
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| ch.get().to_string());
                    (gn, cn)
                };
                let _ = self
                    .voice_state_service
                    .set_user_channel(
                        &discord_user_id,
                        guild_id,
                        ch.get(),
                        guild_name,
                        channel_name,
                    )
                    .await;
            }
            None => {
                let _ = self
                    .voice_state_service
                    .remove_user_from_guild(&discord_user_id, guild_id)
                    .await;
            }
        }

        // Skip announcements for the bot itself
        if new.user_id == ctx.cache.current_user().id {
            return;
        }

        // Collect join/leave events: (channel_id, is_join)
        let old_channel = old.as_ref().and_then(|o| o.channel_id);
        let new_channel = new.channel_id;

        let mut events: Vec<(serenity::ChannelId, bool)> = Vec::new();
        match (old_channel, new_channel) {
            (None, Some(ch)) => {
                events.push((ch, true));
            }
            (Some(ch), None) => {
                events.push((ch, false));
            }
            (Some(old_ch), Some(new_ch)) if old_ch != new_ch => {
                events.push((old_ch, false));
                events.push((new_ch, true));
            }
            _ => {}
        }

        if events.is_empty() {
            return;
        }

        // Extract display name from cache before any .await
        let display_name = {
            let guild = ctx.cache.guild(guild_id_typed);
            guild
                .as_ref()
                .and_then(|g| g.members.get(&new.user_id))
                .map(|m| m.nick.clone().unwrap_or_else(|| m.user.name.clone()))
                .unwrap_or_else(|| discord_user_id.clone())
        };

        if let Err(e) = announce_join_leave(
            &self.service,
            GuildId::from(guild_id),
            DiscordUserId::from(discord_user_id.clone()),
            display_name,
            events,
        )
        .await
        {
            tracing::warn!("Failed to play join/leave announcement for {discord_user_id}: {e}");
        }
    }
}

async fn announce_join_leave(
    service: &Service,
    guild_id: GuildId,
    discord_user_id: DiscordUserId,
    display_name: String,
    events: Vec<(serenity::ChannelId, bool)>,
) -> CoreResult<()> {
    // Fetch user's settings once
    let hq_user = service
        .tap
        .get_user_by_discord_id(&discord_user_id.to_string())
        .await?;

    let settings = if let Some(user) = &hq_user {
        service
            .user_settings
            .get_effective_settings(&user.id, Some(&guild_id.to_string()))
            .await?
    } else {
        UserSettings::default()
    };

    // Resolve tap name from user's TTS voice setting
    let tap_name = resolve_tap_name(service, &settings).await?;

    // Fetch active bot sessions once
    let sessions = service.audio_engine.get_sessions_in_guild(guild_id).await?;

    for (serenity_ch, is_join) in events {
        let channel_id = ChannelId::from(serenity_ch.get());

        // Only announce if bot is in that channel
        if !sessions.iter().any(|s| s.channel_id == channel_id) {
            continue;
        }

        let message = build_message(&settings.user_join_leave_alert, &display_name, is_join);
        let message = match message {
            Some(m) => m,
            None => continue, // UserJoinLeaveAlert::Off
        };

        let queue_name: QueueName = format!("temp-alert-{}", uuid::Uuid::new_v4()).into();

        service
            .audio_engine
            .play(
                guild_id,
                channel_id,
                queue_name,
                tap_name.clone(),
                AudioRequestString::from(message),
                1.0.into(),
                discord_user_id.clone(),
            )
            .await?;
    }

    Ok(())
}

fn build_message(alert: &UserJoinLeaveAlert, display_name: &str, is_join: bool) -> Option<String> {
    match alert {
        UserJoinLeaveAlert::Off => None,
        UserJoinLeaveAlert::Auto => {
            let suffix = if is_join { "등장" } else { "퇴장" };
            Some(format!("{display_name} {suffix}"))
        }
        UserJoinLeaveAlert::WithDifferentUsername(name) => {
            let suffix = if is_join { "등장" } else { "퇴장" };
            Some(format!("{name} {suffix}"))
        }
        UserJoinLeaveAlert::Custom { join_message, leave_message } => {
            if is_join {
                Some(join_message.clone())
            } else {
                Some(leave_message.clone())
            }
        }
    }
}

async fn resolve_tap_name(service: &Service, settings: &UserSettings) -> CoreResult<TapName> {
    match &settings.tts_voice {
        Some(tap_id) => {
            let tap: Option<Tap> = service.tap.get_tap_internal(tap_id.clone()).await?;
            Ok(tap.map(|t| t.name).unwrap_or_else(|| TapName::from("google".to_string())))
        }
        None => Ok(TapName::from("google".to_string())),
    }
}
