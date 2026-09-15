use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ae_protocol::{
    AudioEngineCommand, AudioEngineCommandRequest, AudioEngineCommandResponse,
    AudioEngineSessionCommand, AudioPlayRequest, SessionInfo,
};
use hq_types::{
    AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName, SessionState, TrackId,
    Volume,
    hq::{DiscordUserId, TapId, playback::PlaybackEvent},
};
use opentelemetry::global;
use tokio::sync::broadcast;
use tracing::{instrument, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::{AppConfig, CoreError, CoreResult};

use super::ae_registry::{AeEntry, AeError, AeRegistry, dispatch, rank_engines};
use super::voice_presence::{VoicePresence, VoicePresenceSlot};

fn map_ae_err(e: AeError) -> CoreError {
    match e {
        AeError::AlreadyJoined => CoreError::Conflict("Already in VC".into()),
        AeError::NotJoined => CoreError::InvalidInput("Not in VC".into()),
        AeError::PermissionDenied => CoreError::Forbidden("Permission denied".into()),
        AeError::Tap(t) => CoreError::TapHub(t),
        other => CoreError::Internal(other.to_string()),
    }
}

/// HQ's audio-engine control surface.
///
/// Every audio command the bot issues comes through here. The service owns no
/// session state of its own: it asks the engine registry where the engines are,
/// asks Discord's voice state which bot is sitting in the channel, and
/// dispatches the command to the engine that owns that bot.
#[derive(Clone)]
pub struct AudioEngineService {
    registry: Arc<AeRegistry>,
    presence: VoicePresenceSlot,
    config: Arc<AppConfig>,
    event_tx: broadcast::Sender<PlaybackEvent>,
}

impl AudioEngineService {
    pub fn new(
        registry: Arc<AeRegistry>,
        presence: VoicePresenceSlot,
        config: Arc<AppConfig>,
        event_tx: broadcast::Sender<PlaybackEvent>,
    ) -> Self {
        Self {
            registry,
            presence,
            config,
            event_tx,
        }
    }

    /// The registry, for the RPC surface engines register through.
    pub fn registry(&self) -> Arc<AeRegistry> {
        self.registry.clone()
    }

    /// Every Discord user id that might be one of our bots in a voice channel:
    /// the live engines' bots, plus any configured sub-bot ids.
    ///
    /// The sub-bot ids are reused here rather than duplicated into a second
    /// configuration knob: they are already the ids of Zako's own bots, and
    /// including them means a worker bot whose engine has not registered yet is
    /// still recognised as "ours" rather than mistaken for a user.
    pub async fn zako_bot_ids(&self) -> HashSet<u64> {
        let mut ids: HashSet<u64> = self
            .registry
            .client_ids()
            .await
            .iter()
            .filter_map(|id| id.parse().ok())
            .collect();
        for id in &self.config.sub_bot_ids {
            if let Ok(parsed) = id.parse() {
                ids.insert(parsed);
            }
        }
        ids
    }

    fn presence(&self) -> Option<Arc<dyn VoicePresence>> {
        self.presence.get().cloned()
    }

    /// `(channel, bot)` pairs for our bots in `guild_id`, straight from
    /// Discord's voice state.
    async fn bot_voice_states(&self, guild_id: GuildId) -> Vec<(u64, u64)> {
        let bot_ids = self.zako_bot_ids().await;
        match self.presence() {
            Some(presence) => presence.bot_voice_states(u64::from(guild_id), &bot_ids),
            None => Vec::new(),
        }
    }

    /// The engine serving `(guild_id, channel_id)`, identified by which bot
    /// Discord says is in that channel.
    ///
    /// `None` means "no Zako bot is connected there" — either genuinely nobody,
    /// or the bot's engine has not registered with HQ yet.
    async fn engine_for_channel(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> Option<AeEntry> {
        let bot = self
            .bot_voice_states(guild_id)
            .await
            .into_iter()
            .find(|(channel, _)| *channel == u64::from(channel_id))
            .map(|(_, bot)| bot)?;
        self.registry.entry_by_client_id(&bot.to_string()).await
    }

    /// Bots that are in this guild but in a *different* channel.
    ///
    /// Discord gives a bot token one voice connection per guild, so these
    /// engines cannot take another session there.
    async fn occupied_client_ids(
        &self,
        guild_id: GuildId,
        excluding: ChannelId,
    ) -> HashSet<String> {
        self.bot_voice_states(guild_id)
            .await
            .into_iter()
            .filter(|(channel, _)| *channel != u64::from(excluding))
            .map(|(_, bot)| bot.to_string())
            .collect()
    }

    fn request(
        session: Option<SessionInfo>,
        command: AudioEngineCommand,
    ) -> AudioEngineCommandRequest {
        let mut headers = HashMap::new();
        let cx = tracing::Span::current().context();
        global::get_text_map_propagator(|p| p.inject_context(&cx, &mut headers));
        AudioEngineCommandRequest {
            session,
            command,
            headers,
            idempotency_key: None,
        }
    }

    async fn session_command(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        command: AudioEngineSessionCommand,
    ) -> CoreResult<AudioEngineCommandResponse> {
        let entry = self
            .engine_for_channel(guild_id, channel_id)
            .await
            .ok_or_else(|| CoreError::InvalidInput("Not in VC".into()))?;
        let session = SessionInfo {
            guild_id,
            channel_id,
        };
        dispatch(
            &entry,
            Self::request(Some(session), AudioEngineCommand::SessionCommand(command)),
        )
        .await
        .map_err(map_ae_err)
    }

    #[instrument(skip(self), fields(guild_id = ?guild_id, channel_id = ?channel_id))]
    pub async fn join(&self, guild_id: GuildId, channel_id: ChannelId) -> CoreResult<bool> {
        let session = SessionInfo {
            guild_id,
            channel_id,
        };

        let entries = self.registry.entries().await;
        if entries.is_empty() {
            warn!(
                guild_id = ?guild_id,
                "join requested but no audio engine has registered"
            );
            return Err(CoreError::Internal("No audio engine registered".into()));
        }

        // Placement is deterministic; the only mutable inputs are which bot
        // Discord says is already in the channel and which bots are busy
        // elsewhere in the guild, both read live from the voice state.
        let already_serving = self
            .bot_voice_states(guild_id)
            .await
            .into_iter()
            .find(|(channel, _)| *channel == u64::from(channel_id))
            .map(|(_, bot)| bot.to_string());
        let occupied = self.occupied_client_ids(guild_id, channel_id).await;

        let candidates = rank_engines(&entries, &session, &occupied, already_serving.as_deref());
        if candidates.is_empty() {
            warn!(
                guild_id = ?guild_id,
                total_engines = entries.len(),
                "no eligible audio engine for this guild"
            );
            return Err(CoreError::Internal(
                "No available audio engine for this guild".into(),
            ));
        }

        // Walk the deterministic ranking, stopping at the first engine that
        // accepts. `AlreadyJoined` counts as success: the bot is already in the
        // channel, which is exactly what the caller asked for.
        for candidate in candidates {
            match dispatch(
                &candidate,
                Self::request(Some(session), AudioEngineCommand::Join),
            )
            .await
            {
                Ok(_) | Err(AeError::AlreadyJoined) => {
                    tracing::info!(
                        sink_id = %candidate.sink_id,
                        client_id = %candidate.client_id,
                        guild_id = ?guild_id,
                        channel_id = ?channel_id,
                        "join dispatched"
                    );
                    return Ok(true);
                }
                Err(e) => {
                    warn!(
                        sink_id = %candidate.sink_id,
                        error = %e,
                        "join failed on this engine, trying the next candidate"
                    );
                }
            }
        }

        Err(CoreError::Internal(
            "All audio engines failed to handle Join".into(),
        ))
    }

    #[instrument(skip(self), fields(guild_id = ?guild_id, channel_id = ?channel_id))]
    pub async fn leave(&self, guild_id: GuildId, channel_id: ChannelId) -> CoreResult<bool> {
        let session = SessionInfo {
            guild_id,
            channel_id,
        };

        // Prefer the engine Discord says is actually in the channel. If no bot
        // is there — the bot was kicked out-of-band, or HQ lost track of it —
        // fall back to telling every engine permitted for this guild, so a
        // stale connection gets torn down either way.
        let targets: Vec<AeEntry> = match self.engine_for_channel(guild_id, channel_id).await {
            Some(entry) => vec![entry],
            None => self
                .registry
                .entries()
                .await
                .into_iter()
                .filter(|e| e.is_permitted_for(&guild_id))
                .collect(),
        };

        if targets.is_empty() {
            return Ok(true);
        }

        let request = Self::request(
            Some(session),
            AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Leave),
        );

        let mut last_error = None;
        for entry in targets {
            match dispatch(&entry, request.clone()).await {
                Ok(_) => return Ok(true),
                // Leave is idempotent: an engine that has no such session is
                // already in the state the caller wants.
                Err(AeError::NotJoined) => return Ok(true),
                Err(e) => last_error = Some(e),
            }
        }

        Err(map_ae_err(last_error.unwrap_or(AeError::NoEngine)))
    }

    #[allow(clippy::too_many_arguments)]
    #[instrument(skip(self, audio_request_string), fields(guild_id = ?guild_id, channel_id = ?channel_id, tap_id = %tap_id.0))]
    pub async fn play(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        queue_name: QueueName,
        tap_id: TapId,
        audio_request_string: AudioRequestString,
        volume: Volume,
        discord_user_id: DiscordUserId,
    ) -> CoreResult<()> {
        let play_request = AudioPlayRequest {
            queue_name,
            tap_id,
            ars: audio_request_string,
            volume,
            initiator: discord_user_id,
            headers: {
                let mut headers = HashMap::new();
                let cx = tracing::Span::current().context();
                global::get_text_map_propagator(|p| p.inject_context(&cx, &mut headers));
                headers
            },
        };
        self.session_command(
            guild_id,
            channel_id,
            AudioEngineSessionCommand::Play(play_request),
        )
        .await?;
        let _ = self.event_tx.send(PlaybackEvent::PlaybackChanged);
        Ok(())
    }

    pub async fn set_volume(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        track_id: TrackId,
        volume: Volume,
    ) -> CoreResult<bool> {
        self.session_command(
            guild_id,
            channel_id,
            AudioEngineSessionCommand::SetVolume { track_id, volume },
        )
        .await?;
        Ok(true)
    }

    #[instrument(skip(self), fields(guild_id = ?guild_id))]
    pub async fn stop(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        track_id: TrackId,
    ) -> CoreResult<bool> {
        self.session_command(
            guild_id,
            channel_id,
            AudioEngineSessionCommand::Stop(track_id),
        )
        .await?;
        let _ = self.event_tx.send(PlaybackEvent::PlaybackChanged);
        Ok(true)
    }

    #[instrument(skip(self), fields(guild_id = ?guild_id))]
    pub async fn stop_many(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        filter: AudioStopFilter,
    ) -> CoreResult<bool> {
        self.session_command(
            guild_id,
            channel_id,
            AudioEngineSessionCommand::StopMany(filter),
        )
        .await?;
        let _ = self.event_tx.send(PlaybackEvent::PlaybackChanged);
        Ok(true)
    }

    #[instrument(skip(self), fields(guild_id = ?guild_id))]
    pub async fn next_music(&self, guild_id: GuildId, channel_id: ChannelId) -> CoreResult<bool> {
        self.session_command(guild_id, channel_id, AudioEngineSessionCommand::NextMusic)
            .await?;
        let _ = self.event_tx.send(PlaybackEvent::PlaybackChanged);
        Ok(true)
    }

    #[instrument(skip(self), fields(guild_id = ?guild_id))]
    pub async fn pause(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        queue_name: QueueName,
    ) -> CoreResult<bool> {
        self.session_command(
            guild_id,
            channel_id,
            AudioEngineSessionCommand::Pause(queue_name),
        )
        .await?;
        let _ = self.event_tx.send(PlaybackEvent::PlaybackChanged);
        Ok(true)
    }

    #[instrument(skip(self), fields(guild_id = ?guild_id))]
    pub async fn resume(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        queue_name: QueueName,
    ) -> CoreResult<bool> {
        self.session_command(
            guild_id,
            channel_id,
            AudioEngineSessionCommand::Resume(queue_name),
        )
        .await?;
        let _ = self.event_tx.send(PlaybackEvent::PlaybackChanged);
        Ok(true)
    }

    pub async fn get_session_state(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> CoreResult<SessionState> {
        match self
            .session_command(
                guild_id,
                channel_id,
                AudioEngineSessionCommand::GetSessionState,
            )
            .await?
        {
            AudioEngineCommandResponse::SessionState(state) => Ok(state),
            other => Err(CoreError::Internal(format!(
                "unexpected response for GetSessionState: {other:?}"
            ))),
        }
    }

    /// Every channel in `guild_id` that one of our bots is currently in.
    ///
    /// Membership comes from Discord's voice state, so this is a statement
    /// about what is actually connected rather than about what HQ last asked
    /// for. The queue contents are fetched from the owning engine best-effort:
    /// a channel whose engine is momentarily unreachable still counts as
    /// occupied, because the bot really is in there.
    pub async fn get_sessions_in_guild(&self, guild_id: GuildId) -> CoreResult<Vec<SessionState>> {
        let entries = self.registry.entries().await;
        let mut seen: HashSet<u64> = HashSet::new();
        let mut sessions = Vec::new();

        for (channel, bot) in self.bot_voice_states(guild_id).await {
            if !seen.insert(channel) {
                continue;
            }
            let channel_id = ChannelId::from(channel);
            let state = match entries.iter().find(|e| e.client_id == bot.to_string()) {
                Some(entry) => {
                    let request = Self::request(
                        Some(SessionInfo {
                            guild_id,
                            channel_id,
                        }),
                        AudioEngineCommand::SessionCommand(
                            AudioEngineSessionCommand::GetSessionState,
                        ),
                    );
                    match dispatch(entry, request).await {
                        Ok(AudioEngineCommandResponse::SessionState(state)) => state,
                        other => {
                            warn!(
                                sink_id = %entry.sink_id,
                                guild_id = ?guild_id,
                                channel_id = ?channel_id,
                                response = ?other,
                                "could not read session state; reporting the channel as occupied"
                            );
                            SessionState {
                                guild_id,
                                channel_id,
                                queues: Default::default(),
                            }
                        }
                    }
                }
                None => SessionState {
                    guild_id,
                    channel_id,
                    queues: Default::default(),
                },
            };
            sessions.push(state);
        }

        Ok(sessions)
    }

    /// Discord user ids of every live audio engine's bot.
    pub async fn list_bot_ids(&self) -> CoreResult<Vec<String>> {
        Ok(self.registry.client_ids().await)
    }
}
