use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use hq_types::{
    AudioMetadata, AudioRequestString, ChannelId, GuildId, QueueName, TapHubError, TrackId, Volume,
    hq::{
        DiscordUserId, TapId, UserSettings,
        playback::{
            AudioMetadataDto, DiscordUserInfoDto, EditQueueDto, GuildPlaybackStateDto,
            PlaybackActionDto, QueueMetaDto, TrackDto,
        },
        settings::TextReadingRule,
    },
};
use zako3_states::VoiceStateService;

use crate::{
    CoreError, CoreResult,
    repo::{CreatePlaybackAction, PlaybackAction, PlaybackActionRepo},
    service::{AudioEngineService, DiscordNameResolverSlot},
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
        AudioMetadata::Url(v) => ("url", v.clone()),
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

            let queue_meta = state
                .queues
                .keys()
                .map(|q| {
                    let name = q.to_string();
                    let discord_user_id: Option<u64> = name
                        .strip_prefix("tts_")
                        .and_then(|s| s.parse().ok())
                        .or_else(|| {
                            name.strip_prefix("temp-")
                                .and_then(|s| s.split('-').next())
                                .and_then(|s| s.parse().ok())
                        });
                    let user = discord_user_id
                        .and_then(|uid| resolver.and_then(|r| r.user_info(uid)))
                        .map(|u| DiscordUserInfoDto {
                            id: u.id.to_string(),
                            name: u.name,
                            avatar_url: u.avatar_url,
                        });
                    (name, QueueMetaDto { user })
                })
                .collect();

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
                            tap_id: t.request.tap_id.0.to_string(),
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
                queue_meta,
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
                        track.request.tap_id.clone(),
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

        self.audio_engine
            .pause(g, c, track.queue_name.clone())
            .await?;

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

        self.audio_engine
            .resume(g, c, track.queue_name.clone())
            .await?;

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

    /// Play one track, retrying exactly once on a transient failure.
    ///
    /// A message a user typed must not disappear because a tap blipped or a
    /// request timed out. One retry after a short backoff covers that; a
    /// refusal or "nobody is listening" is an answer, so it is not retried.
    /// When the message is finally dropped, the reason, the ids that identify
    /// the message and the attempt count go out as one structured log line and
    /// one counter increment.
    #[allow(clippy::too_many_arguments)]
    pub async fn play_with_retry(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        queue_name: QueueName,
        tap_id: TapId,
        audio_request_string: AudioRequestString,
        volume: Volume,
        discord_user_id: &DiscordUserId,
        message_id: Option<&str>,
    ) -> CoreResult<()> {
        let (attempts, result) = attempt_play(|| {
            self.audio_engine.play(
                guild_id,
                channel_id,
                queue_name.clone(),
                tap_id.clone(),
                audio_request_string.clone(),
                volume,
                discord_user_id.clone(),
            )
        })
        .await;

        if let Err(err) = &result {
            PlayDrop {
                dropped_reason: DroppedReason::classify(err),
                guild_id: guild_id.to_string(),
                channel_id: channel_id.to_string(),
                user_id: discord_user_id.0.clone(),
                message_id: message_id.map(str::to_string),
                // The engine mints the track id, so a play that failed has no
                // track to point at. Kept as an explicit field so the log
                // shape does not change once the engine reports one.
                track_id: None,
                attempts,
            }
            .record(err);
        }

        result
    }

    /// Record a message that never reached the engine at all: an empty mapping,
    /// or a user the TTS routing did not place in a channel Zako is in.
    ///
    /// Lives here so the drop vocabulary and its counter have one owner rather
    /// than being re-spelled in the bot.
    pub fn record_dropped(
        &self,
        reason: DroppedReason,
        guild_id: GuildId,
        channel_id: ChannelId,
        discord_user_id: &DiscordUserId,
        message_id: Option<&str>,
    ) {
        record_early_drop(reason, guild_id, channel_id, discord_user_id, message_id);
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
                tracing::info!(
                    "User not in a bot-connected channel, falling back to sending TTS to all channels"
                );
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

// ---------------------------------------------------------------------------
// Play retry and drop accounting
// ---------------------------------------------------------------------------

/// How many times one message is offered to the audio engine before it is
/// dropped: the first attempt plus exactly one retry.
pub const PLAY_MAX_ATTEMPTS: u32 = 2;

/// Backoff between the first attempt and the retry. Deliberately short: a user
/// is waiting on this message, and the engine's taphub client already retries a
/// transport failure twice inside a single attempt.
pub const PLAY_RETRY_BACKOFF: Duration = Duration::from_millis(300);

/// Why a message never made it into the audio queue.
///
/// The first six are the audit's vocabulary. `EngineError` is the fallback for
/// a transport or engine failure that fits none of them, kept separate so a
/// drop is never mislabelled as something it is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DroppedReason {
    /// No tap connection is available, or the tap is unknown.
    TapUnavailable,
    /// The request to taphub timed out.
    RequestTimeout,
    /// The user may not use this tap.
    PermissionDenied,
    /// A session exists with nothing consuming its audio.
    NoConsumer,
    /// Routing placed the message in no channel Zako is in.
    NotInChannel,
    /// Text mapping produced nothing to say.
    MappingEmpty,
    /// Anything else: engine transport, an internal engine failure, a tap
    /// script error.
    EngineError,
}

impl DroppedReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TapUnavailable => "tap_unavailable",
            Self::RequestTimeout => "request_timeout",
            Self::PermissionDenied => "permission_denied",
            Self::NoConsumer => "no_consumer",
            Self::NotInChannel => "not_in_channel",
            Self::MappingEmpty => "mapping_empty",
            Self::EngineError => "engine_error",
        }
    }

    /// Whether offering the same message again could succeed.
    ///
    /// A refusal and "nobody is listening" are answers: asking twice cannot
    /// change them and only spends the user's time. A dead tap and a timeout
    /// can be transient. [`is_retriable`] is the error-level decision that
    /// `attempt_play` actually uses; this table is the reason-level view of it.
    pub fn is_retriable(self) -> bool {
        matches!(
            self,
            Self::TapUnavailable | Self::RequestTimeout | Self::EngineError
        )
    }

    /// Classify a play failure.
    ///
    /// Structured variants carry most of the answer; the string match is only
    /// for the two cases the error types flatten into free text (the taphub
    /// transport's timeout and the "no consumer" response).
    pub fn classify(err: &CoreError) -> Self {
        match err {
            CoreError::TapHub(TapHubError::TapUnavailable)
            | CoreError::TapHub(TapHubError::TapNotFound(_)) => Self::TapUnavailable,
            CoreError::TapHub(TapHubError::PermissionDenied(_)) | CoreError::Forbidden(_) => {
                Self::PermissionDenied
            }
            CoreError::TapHub(TapHubError::Internal(msg)) | CoreError::Internal(msg) => {
                Self::from_message(msg)
            }
            CoreError::InvalidInput(msg) => {
                if msg.to_ascii_lowercase().contains("not in vc") {
                    Self::NotInChannel
                } else {
                    Self::EngineError
                }
            }
            // The engine's own variant: it says the session has nothing that
            // would play this, so it is its own reason and not an engine fault.
            // Without this arm it would classify as `EngineError` and HQ would
            // log `dropped_reason=engine_error` for a no-consumer drop.
            CoreError::NoConsumer(_) => Self::NoConsumer,
            CoreError::TapHub(TapHubError::TapScript { .. }) => Self::EngineError,
            _ => Self::EngineError,
        }
    }

    fn from_message(msg: &str) -> Self {
        let lower = msg.to_ascii_lowercase();
        if lower.contains("timed out") || lower.contains("timeout") {
            Self::RequestTimeout
        } else if lower.contains("no consumer") {
            Self::NoConsumer
        } else if lower.contains("not in vc") {
            Self::NotInChannel
        } else {
            Self::EngineError
        }
    }
}

/// The record of one dropped message.
///
/// `channel_id` is the voice channel the play was addressed to on the play
/// path, and the text channel the message arrived in for an early drop.
/// `track_id` is absent for a play that never got a track back from the
/// engine; the field exists so the log shape stays the same once it can.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayDrop {
    pub dropped_reason: DroppedReason,
    pub guild_id: String,
    pub channel_id: String,
    pub user_id: String,
    pub message_id: Option<String>,
    pub track_id: Option<String>,
    pub attempts: u32,
}

impl PlayDrop {
    /// The drop as one JSON object. The log line carries this same object, so
    /// what a log search shows and what this value says cannot disagree.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "dropped_reason": self.dropped_reason.as_str(),
            "guild_id": self.guild_id,
            "channel_id": self.channel_id,
            "user_id": self.user_id,
            "message_id": self.message_id,
            "track_id": self.track_id,
            "attempts": self.attempts,
        })
    }

    /// Emit the drop as a structured log line and count it.
    pub fn record(&self, err: &CoreError) {
        let payload = self.to_json();
        tracing::warn!(
            dropped_reason = self.dropped_reason.as_str(),
            guild_id = %self.guild_id,
            channel_id = %self.channel_id,
            user_id = %self.user_id,
            message_id = self.message_id.as_deref().unwrap_or(""),
            track_id = self.track_id.as_deref().unwrap_or(""),
            attempts = self.attempts,
            payload = %payload,
            error = %err,
            "message dropped: play failed"
        );
        crate::metrics::record_message_dropped(self.dropped_reason.as_str());
    }
}

/// Record a message that was dropped without asking the engine to play it.
///
/// Logged at `debug` and deliberately **not** counted: `mapping_empty` and
/// `not_in_channel` are ordinary states (a message the mapping rules stripped
/// to nothing, a user who is not in a voice channel), not messages that were
/// lost. Counting them would drown `hq_messages_dropped_total` in normal chat
/// traffic. They are recorded so the audit's "silently disappears" list has no
/// silent entry left.
pub fn record_early_drop(
    reason: DroppedReason,
    guild_id: GuildId,
    channel_id: ChannelId,
    discord_user_id: &DiscordUserId,
    message_id: Option<&str>,
) {
    let drop = PlayDrop {
        dropped_reason: reason,
        guild_id: guild_id.to_string(),
        channel_id: channel_id.to_string(),
        user_id: discord_user_id.0.clone(),
        message_id: message_id.map(str::to_string),
        track_id: None,
        attempts: 0,
    };
    let payload = drop.to_json();
    tracing::debug!(
        dropped_reason = reason.as_str(),
        guild_id = %drop.guild_id,
        channel_id = %drop.channel_id,
        user_id = %drop.user_id,
        message_id = drop.message_id.as_deref().unwrap_or(""),
        track_id = "",
        attempts = 0,
        payload = %payload,
        "message not played"
    );
}

/// Whether offering the same message to the engine again could succeed.
///
/// Only a failure taphub itself reported qualifies. `session.play` asks taphub
/// for metadata *before* it writes the track, so a second attempt cannot
/// duplicate audio on that path.
///
/// Everything the engine answers on its own (`CoreError::Internal`, a
/// transport error) is left alone: the engine may already have queued the
/// track before failing (`reconcile()` runs after the queue write), and
/// nothing dedupes a second one, because `play` mints a fresh track id on
/// every call and no engine reads `AudioEngineCommandRequest.idempotency_key`
/// yet.
pub fn is_retriable(err: &CoreError) -> bool {
    match err {
        CoreError::TapHub(TapHubError::TapUnavailable)
        | CoreError::TapHub(TapHubError::TapNotFound(_))
        | CoreError::TapHub(TapHubError::Internal(_)) => true,
        // A tap script that calls its own failure permanent is an answer.
        CoreError::TapHub(TapHubError::TapScript { try_others, .. }) => *try_others,
        _ => false,
    }
}

/// Runs `attempt` until it succeeds, fails in a way a retry cannot fix, or the
/// one allowed retry has been spent. Returns how many attempts were made.
pub async fn attempt_play<F, Fut>(mut attempt: F) -> (u32, CoreResult<()>)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = CoreResult<()>>,
{
    let mut attempts: u32 = 0;
    loop {
        attempts += 1;
        match attempt().await {
            Ok(()) => {
                crate::metrics::record_play_attempt(attempts, true);
                return (attempts, Ok(()));
            }
            Err(err) => {
                let reason = DroppedReason::classify(&err);
                crate::metrics::record_play_attempt(attempts, false);
                if attempts >= PLAY_MAX_ATTEMPTS || !is_retriable(&err) {
                    return (attempts, Err(err));
                }
                tracing::warn!(
                    attempt = attempts,
                    dropped_reason = reason.as_str(),
                    error = %err,
                    "play attempt failed; retrying once"
                );
                tokio::time::sleep(PLAY_RETRY_BACKOFF).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    };

    use super::*;

    fn timeout_err() -> CoreError {
        CoreError::TapHub(TapHubError::Internal(
            "taphub request timed out after 2 attempts (6000ms each)".into(),
        ))
    }

    fn denied_err() -> CoreError {
        CoreError::TapHub(TapHubError::PermissionDenied("tap-1".into()))
    }

    fn not_in_vc_err() -> CoreError {
        CoreError::InvalidInput("Not in VC".into())
    }

    fn engine_err() -> CoreError {
        CoreError::Internal("audio engine transport error: connection closed".into())
    }

    /// `attempt_play` with a scripted outcome: `failures` failed attempts, then
    /// success. Returns the attempts made and the result.
    async fn run(failures: u32, err: fn() -> CoreError) -> (u32, CoreResult<()>) {
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let (attempts, result) = attempt_play(move || {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst) + 1;
                if n <= failures { Err(err()) } else { Ok(()) }
            }
        })
        .await;
        assert_eq!(attempts, calls.load(Ordering::SeqCst));
        (attempts, result)
    }

    #[tokio::test]
    async fn a_transient_failure_is_retried_exactly_once() {
        let (attempts, result) = run(1, timeout_err).await;
        assert_eq!(attempts, 2);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn a_second_failure_gives_up_after_two_attempts() {
        let (attempts, result) = run(u32::MAX, timeout_err).await;
        assert_eq!(attempts, 2, "one retry, never two");
        assert!(matches!(result, Err(CoreError::TapHub(_))));
    }

    #[tokio::test]
    async fn a_permission_denial_is_not_retried() {
        let (attempts, result) = run(u32::MAX, denied_err).await;
        assert_eq!(attempts, 1);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn not_being_in_the_channel_is_not_retried() {
        let (attempts, result) = run(u32::MAX, not_in_vc_err).await;
        assert_eq!(attempts, 1);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn an_engine_side_failure_is_not_retried() {
        // The engine may have queued the track before it failed, and nothing
        // dedupes a second copy, so this is never re-sent.
        let (attempts, result) = run(u32::MAX, engine_err).await;
        assert_eq!(attempts, 1);
        assert!(result.is_err());
    }

    #[test]
    fn only_taphub_reported_failures_are_retriable() {
        assert!(is_retriable(&timeout_err()));
        assert!(is_retriable(&CoreError::TapHub(TapHubError::TapUnavailable)));
        assert!(is_retriable(&CoreError::TapHub(TapHubError::TapScript {
            reason: "tap hiccup".into(),
            try_others: true,
        })));
        assert!(!is_retriable(&denied_err()));
        assert!(!is_retriable(&not_in_vc_err()));
        assert!(!is_retriable(&CoreError::TapHub(TapHubError::TapScript {
            reason: "permanent".into(),
            try_others: false,
        })));
        assert!(!is_retriable(&engine_err()));
    }

    #[test]
    fn the_reason_table_matches_what_a_retry_could_fix() {
        assert!(DroppedReason::TapUnavailable.is_retriable());
        assert!(DroppedReason::RequestTimeout.is_retriable());
        assert!(!DroppedReason::PermissionDenied.is_retriable());
        assert!(!DroppedReason::NotInChannel.is_retriable());
        assert!(!DroppedReason::NoConsumer.is_retriable());
        assert!(!DroppedReason::MappingEmpty.is_retriable());
    }

    #[test]
    fn tap_and_permission_errors_classify_from_their_variants() {
        assert_eq!(
            DroppedReason::classify(&CoreError::TapHub(TapHubError::TapUnavailable)),
            DroppedReason::TapUnavailable
        );
        assert_eq!(
            DroppedReason::classify(&CoreError::TapHub(TapHubError::PermissionDenied(
                "tap-1".into()
            ))),
            DroppedReason::PermissionDenied
        );
        assert_eq!(
            DroppedReason::classify(&CoreError::Forbidden("Permission denied".into())),
            DroppedReason::PermissionDenied
        );
        assert_eq!(
            DroppedReason::classify(&CoreError::Internal("No consumer attached".into())),
            DroppedReason::NoConsumer
        );
    }

    /// The engine now sends the cause as its own error variant, so the reason
    /// has to come from there rather than from the message text. This is the
    /// link the audio audit asked for: a play refused for lack of a consumer is
    /// dropped under `dropped_reason = "no_consumer"`, not `engine_error`.
    #[test]
    fn a_structured_no_consumer_error_classifies_as_no_consumer() {
        use hq_types::NoConsumerReason;

        for reason in [
            NoConsumerReason::ManagerMissing,
            NoConsumerReason::CallLockTimeout,
            NoConsumerReason::NoConnection,
            NoConsumerReason::MixerDead,
        ] {
            assert_eq!(
                DroppedReason::classify(&CoreError::NoConsumer(reason)),
                DroppedReason::NoConsumer,
                "{reason:?} must be dropped as no_consumer"
            );
        }
        assert!(!is_retriable(&CoreError::NoConsumer(
            NoConsumerReason::CallLockTimeout
        )));
    }

    #[test]
    fn the_drop_record_carries_every_documented_field() {
        let drop = PlayDrop {
            dropped_reason: DroppedReason::TapUnavailable,
            guild_id: "1444631500095094847".into(),
            channel_id: "1444631500095094849".into(),
            user_id: "462936250278739998".into(),
            message_id: Some("1444631500095094851".into()),
            track_id: None,
            attempts: 2,
        };

        let json = drop.to_json();
        let obj = json.as_object().expect("drop record is a JSON object");
        for field in [
            "dropped_reason",
            "guild_id",
            "channel_id",
            "user_id",
            "message_id",
            "track_id",
            "attempts",
        ] {
            assert!(obj.contains_key(field), "missing field {field}: {json}");
        }
        assert_eq!(obj["dropped_reason"], "tap_unavailable");
        assert_eq!(obj["attempts"], 2);
    }
}
