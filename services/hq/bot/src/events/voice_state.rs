use std::sync::Arc;

use hq_core::{CoreResult, PlaybackEvent, Service};
use hq_types::{
    AudioRequestString, ChannelId, GuildId, QueueName,
    hq::{DiscordUserId, Tap, TapName, UserJoinLeaveAlert, UserSettings},
};
use poise::serenity_prelude as serenity;
use serenity::{Context, EventHandler, async_trait, model::voice::VoiceState};
use tokio::sync::broadcast;
use zako3_states::VoiceStateService;

use crate::commands::voice::bot_join_and_announce;

pub struct VoiceStateHandler {
    pub voice_state_service: VoiceStateService,
    pub service: Arc<Service>,
    pub event_tx: broadcast::Sender<PlaybackEvent>,
}

struct ChannelSnapshot {
    serenity_channel_id: serenity::ChannelId,
    serenity_guild_id: serenity::GuildId,
    channel_id: ChannelId,
    guild_id: GuildId,
    real_user_count: usize,
}

// ---------------------------------------------------------------------------
// EventHandler impl
// ---------------------------------------------------------------------------

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
        let guild_id_typed = match new.guild_id {
            Some(g) => g,
            None => return,
        };
        let guild_id = guild_id_typed.get();
        let discord_user_id = new.user_id.to_string();

        update_tracking(&self.voice_state_service, &ctx, guild_id_typed, &new).await;

        if is_bot(&ctx, new.user_id) {
            return;
        }

        let _ = self.event_tx.send(PlaybackEvent::VoiceStateChanged);

        let old_ch = old.as_ref().and_then(|o| o.channel_id);
        let new_ch = new.channel_id;

        let events = join_leave_events(old_ch, new_ch);
        if !events.is_empty() {
            let display_name =
                get_display_name(&ctx, guild_id_typed, new.user_id, &discord_user_id);
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

        if let Err(e) = handle_auto_leave_rejoin(
            &self.service,
            &ctx,
            guild_id_typed,
            guild_id,
            affected_channels(old_ch, new_ch),
        )
        .await
        {
            tracing::warn!("Auto-leave/rejoin error for guild {guild_id}: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Voice state tracking
// ---------------------------------------------------------------------------

async fn update_tracking(
    voice_state_service: &VoiceStateService,
    ctx: &Context,
    guild_id_typed: serenity::GuildId,
    new: &VoiceState,
) {
    let discord_user_id = new.user_id.to_string();
    let guild_id = guild_id_typed.get();

    match new.channel_id {
        Some(ch) => {
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
            let _ = voice_state_service
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
            let _ = voice_state_service
                .remove_user_from_guild(&discord_user_id, guild_id)
                .await;
        }
    }
}

// ---------------------------------------------------------------------------
// Pure sync helpers
// ---------------------------------------------------------------------------

fn is_bot(ctx: &Context, user_id: serenity::UserId) -> bool {
    ctx.cache.user(user_id).map(|u| u.bot).unwrap_or(false)
}

fn get_display_name(
    ctx: &Context,
    guild_id: serenity::GuildId,
    user_id: serenity::UserId,
    fallback: &str,
) -> String {
    ctx.cache
        .guild(guild_id)
        .as_ref()
        .and_then(|g| g.members.get(&user_id))
        .map(|m| {
            m.nick
                .clone()
                .or_else(|| m.user.global_name.clone())
                .unwrap_or_else(|| m.user.name.clone())
        })
        .unwrap_or_else(|| fallback.to_string())
}

/// Returns join/leave events derived from the channel transition.
/// Each entry is `(channel_id, is_join)`.
fn join_leave_events(
    old_ch: Option<serenity::ChannelId>,
    new_ch: Option<serenity::ChannelId>,
) -> Vec<(serenity::ChannelId, bool)> {
    match (old_ch, new_ch) {
        (None, Some(ch)) => vec![(ch, true)],
        (Some(ch), None) => vec![(ch, false)],
        (Some(old), Some(new)) if old != new => vec![(old, false), (new, true)],
        _ => vec![],
    }
}

/// Returns the deduplicated set of channels affected by this voice state change.
/// Includes both old and new channel (handles deafen/mute in same channel).
fn affected_channels(
    old_ch: Option<serenity::ChannelId>,
    new_ch: Option<serenity::ChannelId>,
) -> Vec<serenity::ChannelId> {
    let mut channels = Vec::new();
    if let Some(ch) = old_ch {
        channels.push(ch);
    }
    if let Some(ch) = new_ch {
        if !channels.contains(&ch) {
            channels.push(ch);
        }
    }
    channels
}

/// Counts non-bot, non-deafened users in `channel_id` from the guild cache.
fn count_real_users(guild: &serenity::Guild, channel_id: serenity::ChannelId) -> usize {
    guild
        .voice_states
        .values()
        .filter(|vs| vs.channel_id == Some(channel_id))
        .filter(|vs| !vs.deaf && !vs.self_deaf)
        .filter(|vs| {
            guild
                .members
                .get(&vs.user_id)
                .map(|m| !m.user.bot)
                .unwrap_or(true)
        })
        .count()
}

// ---------------------------------------------------------------------------
// Auto-leave / rejoin
// ---------------------------------------------------------------------------

async fn handle_auto_leave_rejoin(
    service: &Service,
    ctx: &Context,
    guild_id_typed: serenity::GuildId,
    guild_id: u64,
    channels: Vec<serenity::ChannelId>,
) -> CoreResult<()> {
    if channels.is_empty() {
        return Ok(());
    }

    let sessions = service
        .audio_engine
        .get_sessions_in_guild(GuildId::from(guild_id))
        .await
        .map(|s| s.into_iter().map(|s| s.channel_id).collect::<Vec<_>>())
        .map_err(|e| {
            tracing::warn!("Failed to fetch audio sessions for guild {guild_id}: {e}");
            e
        })?;

    let snapshots = extract_snapshots(ctx, guild_id_typed, guild_id, &channels);

    for snap in snapshots {
        act_on_snapshot(service, ctx, &sessions, snap).await;
    }

    Ok(())
}

fn extract_snapshots(
    ctx: &Context,
    guild_id_typed: serenity::GuildId,
    guild_id: u64,
    channels: &[serenity::ChannelId],
) -> Vec<ChannelSnapshot> {
    let guild = ctx.cache.guild(guild_id_typed);
    match guild {
        None => vec![],
        Some(g) => channels
            .iter()
            .map(|&serenity_ch| {
                let real_user_count = count_real_users(&g, serenity_ch);
                let channel_id = ChannelId::from(serenity_ch.get());
                tracing::info!(
                    "Channel {serenity_ch} has {real_user_count} real users (guild {guild_id})"
                );
                ChannelSnapshot {
                    serenity_channel_id: serenity_ch,
                    serenity_guild_id: guild_id_typed,
                    channel_id,
                    guild_id: GuildId::from(guild_id),
                    real_user_count,
                }
            })
            .collect(),
    }
}

async fn act_on_snapshot(
    service: &Service,
    ctx: &Context,
    sessions: &[ChannelId],
    snap: ChannelSnapshot,
) {
    let is_intended = service
        .intended_vc
        .contains(u64::from(snap.guild_id), u64::from(snap.channel_id))
        .await
        .unwrap_or(false);

    if !is_intended {
        return;
    }

    let bot_is_present = sessions.contains(&snap.channel_id);

    if snap.real_user_count == 0 && bot_is_present {
        let _ = service
            .audio_engine
            .leave(snap.guild_id, snap.channel_id)
            .await;
        // Keep in intended_vc — will rejoin when someone comes back.
    } else if snap.real_user_count > 0 && !bot_is_present {
        if let Err(e) = bot_join_and_announce(
            service,
            ctx,
            snap.guild_id,
            snap.serenity_guild_id,
            snap.channel_id,
        )
        .await
        {
            tracing::warn!(
                "Failed to auto-rejoin channel {}: {e}",
                snap.serenity_channel_id
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Join/leave announcement
// ---------------------------------------------------------------------------

async fn announce_join_leave(
    service: &Service,
    guild_id: GuildId,
    discord_user_id: DiscordUserId,
    display_name: String,
    events: Vec<(serenity::ChannelId, bool)>,
) -> CoreResult<()> {
    let hq_user = service
        .tap
        .get_user_by_discord_id(&discord_user_id.to_string())
        .await?;

    let user_id_optional = hq_user.as_ref().map(|u| u.id.clone());

    let settings = service
        .user_settings
        .get_effective_settings(&user_id_optional, Some(&guild_id.to_string()))
        .await?;

    let tap_name = resolve_tap_name(service, &settings).await?;
    let sessions = service.audio_engine.get_sessions_in_guild(guild_id).await?;

    for (serenity_ch, is_join) in events {
        let channel_id = ChannelId::from(serenity_ch.get());

        if !sessions.iter().any(|s| s.channel_id == channel_id) {
            continue;
        }

        let Some(message) = build_message(&settings.user_join_leave_alert, &display_name, is_join)
        else {
            continue;
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
    let suffix = |join| if join { "등장" } else { "퇴장" };
    match alert {
        UserJoinLeaveAlert::Off => None,
        UserJoinLeaveAlert::Auto => Some(format!("{display_name} {}", suffix(is_join))),
        UserJoinLeaveAlert::WithDifferentUsername(name) => {
            Some(format!("{name} {}", suffix(is_join)))
        }
        UserJoinLeaveAlert::Custom {
            join_message,
            leave_message,
        } => Some(
            if is_join {
                join_message.replace("{{name}}", display_name)
            } else {
                leave_message.replace("{{name}}", display_name)
            }
            .clone(),
        ),
    }
}

async fn resolve_tap_name(service: &Service, settings: &UserSettings) -> CoreResult<TapName> {
    match &settings.tts_voice {
        Some(tap_id) => {
            let tap: Option<Tap> = service.tap.get_tap(tap_id.clone()).await?;
            Ok(tap
                .map(|t| t.name)
                .unwrap_or_else(|| TapName::from("google".to_string())))
        }
        None => Ok(TapName::from("google".to_string())),
    }
}
