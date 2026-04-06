use std::sync::Arc;

use chrono::Utc;
use hq_types::{
    AudioMetadata, ChannelId, GuildId, QueueName, TrackId, Volume,
    hq::playback::{
        AudioMetadataDto, EditQueueDto, GuildPlaybackStateDto, PlaybackActionDto, TrackDto,
    },
};
use zako3_audio_engine_client::client::AudioEngineRpcClient;
use zako3_states::VoiceStateService;

use crate::{
    CoreError, CoreResult,
    repo::{CreatePlaybackAction, PlaybackAction, PlaybackActionRepo},
};

fn metadata_to_dto(m: &AudioMetadata) -> AudioMetadataDto {
    let (type_str, value) = match m {
        AudioMetadata::Title(v) => ("title", v.clone()),
        AudioMetadata::Description(v) => ("description", v.clone()),
        AudioMetadata::Artist(v) => ("artist", v.clone()),
        AudioMetadata::Album(v) => ("album", v.clone()),
        AudioMetadata::ImageUrl(v) => ("image_url", v.clone()),
    };
    AudioMetadataDto { r#type: type_str.to_string(), value }
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
    audio_engine: Arc<AudioEngineRpcClient>,
    voice_state: VoiceStateService,
    repo: Arc<dyn PlaybackActionRepo>,
}

impl PlaybackService {
    pub fn new(
        audio_engine: Arc<AudioEngineRpcClient>,
        voice_state: VoiceStateService,
        repo: Arc<dyn PlaybackActionRepo>,
    ) -> Self {
        Self { audio_engine, voice_state, repo }
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

            let state = self
                .audio_engine
                .get_session_state(guild_id, channel_id)
                .await
                .map_err(|e| CoreError::Internal(e.to_string()))?;

            let queues = state
                .queues
                .into_iter()
                .map(|(queue_name, tracks)| {
                    let dtos = tracks
                        .iter()
                        .map(|t| TrackDto {
                            track_id: t.track_id.into(),
                            queue_name: t.queue_name.to_string(),
                            metadata: t.metadatas.iter().map(metadata_to_dto).collect(),
                            tap_name: t.request.tap_name.to_string(),
                            audio_request_string: t.request.audio_request.to_string(),
                            requested_by: t.request.discord_user_id.0.clone(),
                            volume: t.volume.into(),
                        })
                        .collect();
                    (queue_name.to_string(), dtos)
                })
                .collect();

            results.push(GuildPlaybackStateDto {
                guild_id: loc.guild_id.to_string(),
                channel_id: loc.channel_id.to_string(),
                queues,
            });
        }

        Ok(results)
    }

    pub async fn stop_track(
        &self,
        guild_id: u64,
        channel_id: u64,
        track_id: u64,
        actor_discord_user_id: &str,
    ) -> CoreResult<PlaybackActionDto> {
        let g = GuildId::from(guild_id);
        let c = ChannelId::from(channel_id);

        let state = self
            .audio_engine
            .get_session_state(g, c)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))?;

        let track = state
            .find_track(TrackId::from(track_id))
            .ok_or_else(|| CoreError::NotFound(format!("track {}", track_id)))?;

        let snapshot = serde_json::to_value(track)?;

        self.audio_engine
            .stop(g, c, TrackId::from(track_id))
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))?;

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

        let state = self
            .audio_engine
            .get_session_state(g, c)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))?;

        let music_queue = QueueName::from("music".to_string());
        let head_track = state
            .queues
            .get(&music_queue)
            .and_then(|q| q.first())
            .ok_or_else(|| CoreError::NotFound("no music track playing".to_string()))?;

        let snapshot = serde_json::to_value(head_track)?;

        self.audio_engine
            .next_music(g, c)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))?;

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

        let state = self
            .audio_engine
            .get_session_state(g, c)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))?;

        let queue_snapshot = serde_json::to_value(&state.queues)?;

        for op in &dto.operations {
            let track_id = TrackId::from(op.track_id);
            match op.op.as_str() {
                "remove" => {
                    self.audio_engine
                        .stop(g, c, track_id)
                        .await
                        .map_err(|e| CoreError::Internal(e.to_string()))?;
                }
                "set_volume" => {
                    let vol = op
                        .volume
                        .ok_or_else(|| CoreError::InvalidInput("volume required for set_volume op".into()))?;
                    self.audio_engine
                        .set_volume(g, c, track_id, Volume::from(vol))
                        .await
                        .map_err(|e| CoreError::Internal(e.to_string()))?;
                }
                other => {
                    return Err(CoreError::InvalidInput(format!("unknown op: {}", other)));
                }
            }
        }

        let first_track_id =
            dto.operations.first().map(|op| op.track_id).unwrap_or(0);
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
                let track: hq_types::Track =
                    serde_json::from_value(action.track_snapshot.clone())?;

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
                    .await
                    .map_err(|e| CoreError::Internal(e.to_string()))?;
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

        let guild_ids: Vec<String> =
            locations.iter().map(|loc| loc.guild_id.to_string()).collect();

        let actions = self.repo.find_by_guild_ids(&guild_ids, limit).await?;

        Ok(actions.iter().map(action_to_dto).collect())
    }
}

