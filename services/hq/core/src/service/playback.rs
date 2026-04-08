use std::sync::Arc;

use chrono::Utc;
use hq_types::{
    hq::{
        playback::{
            AudioMetadataDto, EditQueueDto, GuildPlaybackStateDto, PlaybackActionDto, TrackDto,
        },
        settings::TextReadingRule,
        UserSettings,
    },
    AudioMetadata, ChannelId, GuildId, QueueName, TrackId, Volume,
};
use zako3_states::VoiceStateService;

use crate::{
    repo::{CreatePlaybackAction, PlaybackAction, PlaybackActionRepo},
    service::{AudioEngineService, DiscordNameResolverSlot},
    CoreError, CoreResult,
};

/// Minimal voice state info extracted from a Discord gateway event or cache,
/// used to determine TTS routing without taking a dependency on serenity types.
#[derive(Debug, Clone)]
pub struct UserVoiceInfo {
    pub channel_id: Option<ChannelId>,
    pub mute: bool,
    pub self_mute: bool,
}

fn metadata_to_dto(m: &AudioMetadata) -> AudioMetadataDto {
    let (type_str, value) = match m {
        AudioMetadata::Title(v) => ("title", v.clone()),
        AudioMetadata::Description(v) => ("description", v.clone()),
        AudioMetadata::Artist(v) => ("artist", v.clone()),
        AudioMetadata::Album(v) => ("album", v.clone()),
        AudioMetadata::ImageUrl(v) => ("image_url", v.clone()),
    };
    AudioMetadataDto {
        r#type: type_str.to_string(),
        value,
    }
}

fn action_to_dto(a: &PlaybackAction) -> PlaybackActionDto {
    PlaybackActionDto {
        id: a.id.clone(),
        action_type: a.action_type.clone(),
        guild_id: a.guild_id.clone(),
        channel_id: a.channel_id.clone(),
        actor_discord_user_id: a.actor_discord_user_id.clone(),
        created_at: a.created_at,
        undone_at: a.undone_at,
        undone_by_discord_user_id: a.undone_by_discord_user_id.clone(),
    }
}

#[derive(Clone)]
pub struct PlaybackService {
    audio_engine: AudioEngineService,
    voice_state: VoiceStateService,
    repo: Arc<dyn PlaybackActionRepo>,
    name_resolver_slot: DiscordNameResolverSlot,
}

impl PlaybackService {
    pub fn new(
        audio_engine: AudioEngineService,
        voice_state: VoiceStateService,
        repo: Arc<dyn PlaybackActionRepo>,
        name_resolver_slot: DiscordNameResolverSlot,
    ) -> Self {
        Self {
            audio_engine,
            voice_state,
            repo,
            name_resolver_slot,
        }
    }

    pub async fn get_state_for_user(
        &self,
        discord_user_id: &str,
    ) -> CoreResult<Vec<GuildPlaybackStateDto>> {
        let locations = self
            .voice_state
            .get_user_channels(discord_user_id)
            .await
            .map_err(CoreError::StateError)?;

        let mut results = Vec::new();
        for loc in locations {
            let guild_id = GuildId::from(loc.guild_id);
            let channel_id = ChannelId::from(loc.channel_id);

            let state = match self
                .audio_engine
                .get_session_state(guild_id, channel_id)
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::debug!(guild_id = %loc.guild_id, channel_id = %loc.channel_id, "No audio session for channel, skipping: {}", e);
                    continue;
                }
            };

            let resolver = self.name_resolver_slot.get();
            let guild_name = resolver
                .and_then(|r| r.guild_name(loc.guild_id))
                .unwrap_or_else(|| loc.guild_name.clone());
            let channel_name = resolver
                .and_then(|r| r.channel_name(loc.guild_id, loc.channel_id))
                .unwrap_or_else(|| loc.channel_name.clone());
            let guild_icon_url = resolver.and_then(|r| r.guild_icon_url(loc.guild_id));

            let queues = state
                .queues
                .into_iter()
                .map(|(queue_name, tracks)| {
                    let dtos = tracks
                        .iter()
                        .map(|t| TrackDto {
                            track_id: t.track_id.to_string(),
                            queue_name: t.queue_name.to_string(),
                            metadata: t.metadatas.iter().map(metadata_to_dto).collect(),
                            tap_name: t.request.tap_name.to_string(),
                            audio_request_string: t.request.audio_request.to_string(),
                            requested_by: t.request.discord_user_id.0.clone(),
                            volume: t.volume.into(),
                            paused: t.paused,
                        })
                        .collect();
                    (queue_name.to_string(), dtos)
                })
                .collect();

            results.push(GuildPlaybackStateDto {
                guild_id: loc.guild_id.to_string(),
                guild_name,
                guild_icon_url,
                channel_id: loc.channel_id.to_string(),
                channel_name,
                queues,
            });
        }

        Ok(results)
    }

    pub async fn stop_track(
        &self,
        guild_id: u64,
        channel_id: u64,
        track_id: &str,
        actor_discord_user_id: &str,
    ) -> CoreResult<PlaybackActionDto> {
        let tid: u64 = track_id
            .parse()
            .map_err(|_| CoreError::InvalidInput("invalid track_id".into()))?;
        let g = GuildId::from(guild_id);
        let c = ChannelId::from(channel_id);

        let state = self.audio_engine.get_session_state(g, c).await?;

        let track = state
            .find_track(TrackId::from(tid))
            .ok_or_else(|| CoreError::NotFound(format!("track {}", tid)))?;

        let snapshot = serde_json::to_value(track)?;

        self.audio_engine.stop(g, c, TrackId::from(tid)).await?;

        let action = self
            .repo
            .create(&CreatePlaybackAction {
                action_type: "stop".to_string(),
                guild_id: guild_id.to_string(),
                channel_id: channel_id.to_string(),
                actor_discord_user_id: actor_discord_user_id.to_string(),
                track_snapshot: snapshot,
                queue_snapshot: None,
            })
            .await?;

        Ok(action_to_dto(&action))
    }

    pub async fn skip_music(
        &self,
        guild_id: u64,
        channel_id: u64,
        actor_discord_user_id: &str,
    ) -> CoreResult<PlaybackActionDto> {
        let g = GuildId::from(guild_id);
        let c = ChannelId::from(channel_id);

        let state = self.audio_engine.get_session_state(g, c).await?;

        let music_queue = QueueName::from("music".to_string());
        let head_track = state
            .queues
            .get(&music_queue)
            .and_then(|q| q.first())
            .ok_or_else(|| CoreError::NotFound("no music track playing".to_string()))?;

        let snapshot = serde_json::to_value(head_track)?;

        self.audio_engine.next_music(g, c).await?;

        let action = self
            .repo
            .create(&CreatePlaybackAction {
                action_type: "skip".to_string(),
                guild_id: guild_id.to_string(),
                channel_id: channel_id.to_string(),
                actor_discord_user_id: actor_discord_user_id.to_string(),
                track_snapshot: snapshot,
                queue_snapshot: None,
            })
            .await?;

        Ok(action_to_dto(&action))
    }

    pub async fn edit_queue(
        &self,
        dto: EditQueueDto,
        actor_discord_user_id: &str,
    ) -> CoreResult<PlaybackActionDto> {
        let guild_id: u64 = dto
            .guild_id
            .parse()
            .map_err(|_| CoreError::InvalidInput("invalid guild_id".into()))?;
        let channel_id: u64 = dto
            .channel_id
            .parse()
            .map_err(|_| CoreError::InvalidInput("invalid channel_id".into()))?;

        let g = GuildId::from(guild_id);
        let c = ChannelId::from(channel_id);

        let state = self.audio_engine.get_session_state(g, c).await?;

        let queue_snapshot = serde_json::to_value(&state.queues)?;

        for op in &dto.operations {
            let tid: u64 = op
                .track_id
                .parse()
                .map_err(|_| CoreError::InvalidInput("invalid track_id".into()))?;
            let track_id = TrackId::from(tid);
            match op.op.as_str() {
                "remove" => {
                    self.audio_engine.stop(g, c, track_id).await?;
                }
                "set_volume" => {
                    let vol = op.volume.ok_or_else(|| {
                        CoreError::InvalidInput("volume required for set_volume op".into())
                    })?;
                    self.audio_engine
                        .set_volume(g, c, track_id, Volume::from(vol))
                        .await?;
                }
                other => {
                    return Err(CoreError::InvalidInput(format!("unknown op: {}", other)));
                }
            }
        }

        let first_track_id = dto
            .operations
            .first()
            .map(|op| op.track_id.as_str())
            .unwrap_or("");
        let track_snapshot = serde_json::json!({ "first_track_id": first_track_id });

        let action = self
            .repo
            .create(&CreatePlaybackAction {
                action_type: "edit_queue".to_string(),
                guild_id: dto.guild_id.clone(),
                channel_id: dto.channel_id.clone(),
                actor_discord_user_id: actor_discord_user_id.to_string(),
                track_snapshot,
                queue_snapshot: Some(queue_snapshot),
            })
            .await?;

        Ok(action_to_dto(&action))
    }

    pub async fn undo_action(
        &self,
        action_id: &str,
        actor_discord_user_id: &str,
    ) -> CoreResult<PlaybackActionDto> {
        let action = self
            .repo
            .find_by_id(action_id)
            .await?
            .ok_or_else(|| CoreError::NotFound(format!("action {}", action_id)))?;

        if action.undone_at.is_some() {
            return Err(CoreError::Conflict("action already undone".into()));
        }

        let guild_id: u64 = action
            .guild_id
            .parse()
            .map_err(|_| CoreError::Internal("invalid guild_id in DB".into()))?;
        let channel_id: u64 = action
            .channel_id
            .parse()
            .map_err(|_| CoreError::Internal("invalid channel_id in DB".into()))?;

        match action.action_type.as_str() {
            "stop" | "skip" => {
                let track: hq_types::Track = serde_json::from_value(action.track_snapshot.clone())?;

                self.audio_engine
                    .play(
                        GuildId::from(guild_id),
                        ChannelId::from(channel_id),
                        track.queue_name.clone(),
                        track.request.tap_name.clone(),
                        track.request.audio_request.clone(),
                        track.volume,
                        track.request.discord_user_id.clone(),
                    )
                    .await?;
            }
            "edit_queue" => {
                return Err(CoreError::InvalidInput(
                    "edit_queue undo is not supported".into(),
                ));
            }
            other => {
                return Err(CoreError::InvalidInput(format!(
                    "unknown action type: {}",
                    other
                )));
            }
        }

        let updated = self
            .repo
            .mark_undone(action_id, actor_discord_user_id, Utc::now())
            .await?;

        Ok(action_to_dto(&updated))
    }

    pub async fn get_history(
        &self,
        discord_user_id: &str,
        limit: i64,
    ) -> CoreResult<Vec<PlaybackActionDto>> {
        let locations = self
            .voice_state
            .get_user_channels(discord_user_id)
            .await
            .map_err(CoreError::StateError)?;

        let guild_ids: Vec<String> = locations
            .iter()
            .map(|loc| loc.guild_id.to_string())
            .collect();

        let actions = self.repo.find_by_guild_ids(&guild_ids, limit).await?;

        Ok(actions.iter().map(action_to_dto).collect())
    }

    pub async fn pause_track(
        &self,
        guild_id: u64,
        channel_id: u64,
        track_id: &str,
        actor_discord_user_id: &str,
    ) -> CoreResult<PlaybackActionDto> {
        let tid: u64 = track_id
            .parse()
            .map_err(|_| CoreError::InvalidInput("invalid track_id".into()))?;
        let g = GuildId::from(guild_id);
        let c = ChannelId::from(channel_id);

        let state = self.audio_engine.get_session_state(g, c).await?;

        let track = state
            .find_track(TrackId::from(tid))
            .ok_or_else(|| CoreError::NotFound(format!("track {}", tid)))?;

        let snapshot = serde_json::to_value(track)?;

        self.audio_engine.pause(g, c, TrackId::from(tid)).await?;

        let action = self
            .repo
            .create(&CreatePlaybackAction {
                action_type: "pause".to_string(),
                guild_id: guild_id.to_string(),
                channel_id: channel_id.to_string(),
                actor_discord_user_id: actor_discord_user_id.to_string(),
                track_snapshot: snapshot,
                queue_snapshot: None,
            })
            .await?;

        Ok(action_to_dto(&action))
    }

    pub async fn resume_track(
        &self,
        guild_id: u64,
        channel_id: u64,
        track_id: &str,
        actor_discord_user_id: &str,
    ) -> CoreResult<PlaybackActionDto> {
        let tid: u64 = track_id
            .parse()
            .map_err(|_| CoreError::InvalidInput("invalid track_id".into()))?;
        let g = GuildId::from(guild_id);
        let c = ChannelId::from(channel_id);

        let state = self.audio_engine.get_session_state(g, c).await?;

        let track = state
            .find_track(TrackId::from(tid))
            .ok_or_else(|| CoreError::NotFound(format!("track {}", tid)))?;

        let snapshot = serde_json::to_value(track)?;

        self.audio_engine.resume(g, c, TrackId::from(tid)).await?;

        let action = self
            .repo
            .create(&CreatePlaybackAction {
                action_type: "resume".to_string(),
                guild_id: guild_id.to_string(),
                channel_id: channel_id.to_string(),
                actor_discord_user_id: actor_discord_user_id.to_string(),
                track_snapshot: snapshot,
                queue_snapshot: None,
            })
            .await?;

        Ok(action_to_dto(&action))
    }

    /// Determines which voice channels should receive TTS audio given the user's
    /// voice state and their `TextReadingRule` setting.
    ///
    /// Mirrors the routing logic previously in `hq-bot`'s `message_create` handler,
    /// now centralised here so slash commands can reuse it.
    pub async fn resolve_tts_channels(
        &self,
        guild_id: GuildId,
        message_channel_id: ChannelId,
        user_voice_info: Option<UserVoiceInfo>,
        settings: &UserSettings,
    ) -> CoreResult<Vec<ChannelId>> {
        let bot_channel_ids = self
            .audio_engine
            .get_sessions_in_guild(guild_id)
            .await?
            .into_iter()
            .map(|s| s.channel_id)
            .collect::<Vec<_>>();

        if bot_channel_ids.contains(&message_channel_id) {
            return Ok(vec![message_channel_id]);
        }

        tracing::info!(
            "Bot channel ids in guild {}: {:?}",
            guild_id,
            bot_channel_ids
        );

        match settings.text_reading_rule {
            TextReadingRule::Always => {
                if let Some(info) = user_voice_info {
                    if let Some(channel_id) = info.channel_id {
                        if bot_channel_ids.contains(&channel_id) {
                            tracing::info!(
                                "User in bot-connected channel {}, routing TTS there",
                                channel_id
                            );
                            return Ok(vec![channel_id]);
                        }
                    }
                }
                tracing::info!("User not in a bot-connected channel, falling back to sending TTS to all channels");
                Ok(bot_channel_ids)
            }
            TextReadingRule::InVoiceChannel | TextReadingRule::OnMicMute => {
                tracing::info!("currently unreachable");
                if let Some(info) = user_voice_info {
                    if let Some(channel_id) = info.channel_id {
                        if !bot_channel_ids.contains(&channel_id) {
                            return Ok(vec![]);
                        }

                        match settings.text_reading_rule {
                            TextReadingRule::InVoiceChannel => return Ok(vec![channel_id]),
                            TextReadingRule::OnMicMute => {
                                if info.mute || info.self_mute {
                                    return Ok(vec![channel_id]);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Ok(vec![])
            }
        }
    }
}
