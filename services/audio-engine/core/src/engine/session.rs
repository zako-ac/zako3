use std::{
    collections::HashMap,
    sync::atomic::AtomicBool,
    sync::{Arc, Weak},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use tokio::sync::{Mutex, mpsc::Sender};
use tracing::instrument;
use zako3_audio_engine_audio::metrics;
use zako3_types::{NoConsumerReason, SessionState};

use crate::{
    audio::{ArcDecoder, ArcMixer},
    error::ZakoResult,
    service::{ArcStateService, ArcTapHubService, modify_state_session},
    types::{
        AudioRequest, AudioRequestString, AudioStopFilter, CachedAudioRequest, ChannelId, GuildId,
        QueueName, Track, TrackId, Volume,
    },
    util::id_gen,
};
use zako3_types::hq::{DiscordUserId, TapId};

/// Default for [`SessionConfig::track_watchdog`].
pub const DEFAULT_TRACK_WATCHDOG_SECS: u64 = 60;

/// Default for [`SessionConfig::queue_max_age`]: how long a queued TTS may wait
/// for its turn before it is dropped instead of played.
///
/// 90s is chosen from what the queues actually look like. The median TTS track
/// is 1.78s, so a healthy per-user queue drains in single-digit seconds even at
/// several messages a second, and 90s is roughly fifty tracks deep: a user
/// would have to be fifty messages behind before it fires on their own traffic.
/// What it has to catch is the pathological case the audit found: a stalled
/// track holding a user's queue until the backlog reaches 15+ messages, where
/// the tail would have been played a minute or two after it was said. Nothing
/// that late is worth hearing (it answers a question the user has moved past),
/// and dropping it is what lets the fresh messages behind it through.
///
/// Settable without a code change: `QUEUE_MAX_AGE_SECS` in the environment
/// (chart value `audioEngine.queueMaxAgeSecs`). Zero disables the cap. Tune it
/// against `audio_queue_stale_skipped_total` plus the `Dropping stale queued
/// track` log (which carries the age): raise it if users report messages that
/// were never read, lower it if audio still arrives too late to be useful.
pub const DEFAULT_QUEUE_MAX_AGE_SECS: u64 = 90;

/// How many stale heads one `reconcile` will drop before it hands the rest to
/// the next one, so a backlog of thousands cannot make a single reconcile loop.
const MAX_STALE_SKIP_ROUNDS: usize = 8;

/// Knobs for one session's control loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionConfig {
    /// How long a track may run without its end event before the watchdog
    /// stops it. Zero disables the watchdog.
    ///
    /// This is the window in which a track makes *no progress at all*, not the
    /// length of the track: a stalled track is stopped after this long, a long
    /// song that is still producing audio is not touched however far past the
    /// window it runs.
    pub track_watchdog: Duration,

    /// How long a queued track may wait for its turn before it is dropped
    /// instead of played. Zero disables the cap. See
    /// [`DEFAULT_QUEUE_MAX_AGE_SECS`] for why 90 seconds and how to tune it.
    ///
    /// This is the answer to a stalled queue that the watchdog cannot give: the
    /// watchdog ends the stuck *track*, but by then the messages behind it have
    /// already waited, and playing them minutes late is worse than skipping
    /// them.
    pub queue_max_age: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            track_watchdog: Duration::from_secs(DEFAULT_TRACK_WATCHDOG_SECS),
            queue_max_age: Duration::from_secs(DEFAULT_QUEUE_MAX_AGE_SECS),
        }
    }
}

/// What the watchdog needs to say whose stuck track it just stopped.
#[derive(Debug, Clone)]
struct StartedTrack {
    started_at: Instant,
    queue_name: QueueName,
    tap_id: TapId,
    discord_user_id: DiscordUserId,
}

pub struct SessionControl {
    pub guild_id: GuildId,
    pub channel_id: ChannelId,

    pub(crate) mixer: ArcMixer,
    pub(crate) decoder: ArcDecoder,

    pub(crate) end_tx: Sender<TrackId>,

    pub(crate) state_service: ArcStateService,
    pub(crate) taphub_service: ArcTapHubService,

    /// Liveness of this session's mixer thread. `false` once it has exited (its
    /// output consumer went away), which means nothing will ever pull audio out
    /// of this session again, a state the session must refuse to play into
    /// rather than queue silently (see `ensure_consumer`).
    pub(crate) mixer_alive: Arc<AtomicBool>,

    // Serializes reconcile() to prevent concurrent check-then-act races where
    // two callers both see a track as "not in mixer" and both call play_now().
    reconcile_guard: Mutex<()>,

    config: SessionConfig,

    // Tracks the mixer has been told to play whose end event has not arrived
    // yet. The watchdog reads this to find tracks that overstay.
    started: Mutex<HashMap<TrackId, StartedTrack>>,
}

impl SessionControl {
    // The session owns one handle per collaborator; grouping them into a struct
    // would only move the count somewhere else.
    #[allow(clippy::too_many_arguments)]
    fn new(
        guild_id: GuildId,
        channel_id: ChannelId,
        end_tx: Sender<TrackId>,
        mixer: ArcMixer,
        decoder: ArcDecoder,
        state_service: ArcStateService,
        taphub_service: ArcTapHubService,
        config: SessionConfig,
        mixer_alive: Arc<AtomicBool>,
    ) -> Self {
        SessionControl {
            guild_id,
            channel_id,
            mixer,
            decoder,
            end_tx,
            state_service,
            taphub_service,
            mixer_alive,
            reconcile_guard: Mutex::new(()),
            config,
            started: Mutex::new(HashMap::new()),
        }
    }

    /// Refuse to queue audio when this session has nothing that would play it.
    ///
    /// The old behaviour was to accept the play and answer `Ok`: the track sat
    /// in the queue, the mixer thread was gone, and the channel went silent
    /// while the caller believed the message had been read. Failing here is what
    /// turns that silence into a visible play failure at HQ.
    /// Whether this session still has something that will play what it is given.
    ///
    /// `false` means the mixer thread is gone: the session is registered and its
    /// queue is readable, but nothing it accepts would ever be heard. `Join`
    /// uses this to repair such a session instead of answering `AlreadyJoined`.
    pub fn has_consumer(&self) -> bool {
        self.mixer_alive.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn ensure_consumer(&self) -> ZakoResult<()> {
        if self.mixer_alive.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok(());
        }

        tracing::error!(
            guild_id = %self.guild_id,
            channel_id = %self.channel_id,
            reason = NoConsumerReason::MixerDead.as_str(),
            "no audio consumer: the session's mixer thread has exited"
        );
        Err(crate::error::ZakoError::NoConsumer(
            NoConsumerReason::MixerDead,
        ))
    }

    #[instrument(skip(self), fields(guild_id = %self.guild_id))]
    pub async fn play(
        self: &Arc<Self>,
        queue_name: QueueName,
        tap_id: TapId,
        request: AudioRequestString,
        volume: Volume,
        discord_user_id: zako3_types::hq::DiscordUserId,
    ) -> ZakoResult<TrackId> {
        tracing::info!(
            queue_name = %queue_name,
            tap_id = %tap_id.0,
            volume = %volume,
            discord_user_id = %discord_user_id,
            "Playing audio"
        );

        // Before the taphub round trip: there is no point spending a metadata
        // request on audio that could not be played, and the caller has to learn
        // now that this channel cannot play anything.
        self.ensure_consumer()?;

        let track_id: TrackId = id_gen::generate_id();

        let ar = AudioRequest {
            tap_id: tap_id.clone(),
            request: request.clone(),
            discord_user_id: discord_user_id.clone(),
            headers: Default::default(),
        };

        let meta = self.taphub_service.request_audio_meta(ar.clone()).await?;
        tracing::info!("base_volume = {}", meta.base_volume);

        let effective_volume = Volume::from(f32::from(volume) * meta.base_volume);

        let queue_name_for_metric = queue_name.clone();
        modify_state_session(
            &self.state_service,
            self.guild_id,
            self.channel_id,
            move |session| {
                let track = Track {
                    track_id,
                    metadatas: meta.metadatas,
                    request: CachedAudioRequest {
                        tap_id,
                        audio_request: request,
                        cache_key: meta.cache_key,
                        discord_user_id,
                        headers: Default::default(),
                    },
                    volume: effective_volume,
                    queue_name: queue_name.clone(),
                    paused: false,
                    queued_at_ms: Some(now_unix_ms()),
                };

                upsert_track(&mut session.queues, queue_name.clone(), track);
            },
        )
        .await?;

        metrics::record_track_lifecycle("queued", &normalize_queue_name(&queue_name_for_metric));

        // The mixer can exit during the taphub round trip above. Re-checking
        // here is what keeps the window between "it was alive" and "the track is
        // in the queue" from becoming another silent success: `add_source`
        // swallows its send error, so nothing downstream would notice.
        if let Err(e) = self.ensure_consumer() {
            let tid = track_id;
            modify_state_session(
                &self.state_service,
                self.guild_id,
                self.channel_id,
                move |session| session.remove_track(tid),
            )
            .await?;
            return Err(e);
        }

        self.reconcile().await?;

        let this = Arc::clone(self);
        tokio::spawn(async move {
            if let Err(e) = this.preload_if_possible(track_id).await {
                tracing::warn!(track_id = %track_id, error = %e, "Background preload failed");
            }
        });

        Ok(track_id)
    }

    #[instrument(skip(self), fields(guild_id = %self.guild_id))]
    pub async fn set_volume(&self, track_id: TrackId, volume: Volume) -> ZakoResult<()> {
        tracing::debug!(track_id = %track_id, volume = %volume, "Setting volume");
        self.mixer.set_volume(track_id, volume.into());
        modify_state_session(
            &self.state_service,
            self.guild_id,
            self.channel_id,
            move |session| {
                if let Some(track) = session.find_track_mut(track_id) {
                    track.volume = volume;
                }
            },
        )
        .await?;
        Ok(())
    }

    #[instrument(skip(self), fields(guild_id = %self.guild_id))]
    pub async fn pause(&self, track_id: TrackId) -> ZakoResult<()> {
        tracing::info!(track_id = %track_id, "Pausing track");
        self.decoder.pause_track(track_id);
        modify_state_session(
            &self.state_service,
            self.guild_id,
            self.channel_id,
            move |session| {
                if let Some(track) = session.find_track_mut(track_id) {
                    track.paused = true;
                }
            },
        )
        .await?;
        Ok(())
    }

    #[instrument(skip(self), fields(guild_id = %self.guild_id))]
    pub async fn resume(&self, track_id: TrackId) -> ZakoResult<()> {
        tracing::info!(track_id = %track_id, "Resuming track");
        modify_state_session(
            &self.state_service,
            self.guild_id,
            self.channel_id,
            move |session| {
                if let Some(track) = session.find_track_mut(track_id) {
                    track.paused = false;
                    // A resume is the user saying "play this now": the time it
                    // spent paused is not time it waited for its turn, so it must
                    // not count against the queue age cap.
                    track.queued_at_ms = Some(now_unix_ms());
                }
            },
        )
        .await?;
        self.decoder.resume_track(track_id);
        Ok(())
    }

    #[instrument(skip(self), fields(guild_id = %self.guild_id))]
    pub async fn stop(&self, track_id: TrackId) -> ZakoResult<()> {
        tracing::info!(track_id = %track_id, "Stopping track");

        let queue_name = self
            .state_service
            .get_session(self.guild_id, self.channel_id)
            .await?
            .and_then(|s| s.find_track(track_id).map(|t| t.queue_name.clone()));

        self.mixer.remove_source(track_id);
        self.decoder.stop_track(track_id);
        self.started.lock().await.remove(&track_id);
        modify_state_session(
            &self.state_service,
            self.guild_id,
            self.channel_id,
            move |session| {
                session.remove_track(track_id);
            },
        )
        .await?;

        if let Some(qn) = queue_name {
            metrics::record_track_lifecycle("stop", &normalize_queue_name(&qn));
        }

        self.reconcile().await?;

        Ok(())
    }

    #[instrument(skip(self), fields(guild_id = %self.guild_id))]
    pub async fn stop_many(&self, filter: AudioStopFilter) -> ZakoResult<()> {
        tracing::info!(filter = ?filter, "Stopping multiple tracks");

        let mut session = self
            .state_service
            .get_session(self.guild_id, self.channel_id)
            .await?;
        let track_ids = session
            .as_ref()
            .map(|s| match filter {
                AudioStopFilter::All => s.get_all_track_ids(),
                AudioStopFilter::Music => s.get_all_track_ids_by_queue_name_prefix("music"),
                AudioStopFilter::TTS(user_id) => {
                    s.get_all_track_ids_by_queue_name_prefix(&format!("tts_{}", user_id))
                }
            })
            .unwrap_or_default();

        let stop_count = track_ids.len();

        for track_id in track_ids {
            self.mixer.remove_source(track_id);
            self.decoder.stop_track(track_id);
            self.started.lock().await.remove(&track_id);

            if let Some(session) = session.as_mut() {
                if let Some(track) = session.find_track(track_id) {
                    metrics::record_track_lifecycle(
                        "stop",
                        &normalize_queue_name(&track.queue_name),
                    );
                }
                session.remove_track(track_id);
            }
        }

        if let Some(session) = session {
            self.state_service.save_session(&session).await?;
        }

        tracing::info!(count = stop_count, "Stopped tracks");

        self.reconcile().await?;

        Ok(())
    }

    #[instrument(skip(self), fields(guild_id = %self.guild_id))]
    pub async fn next_music(self: &Arc<Self>) -> ZakoResult<()> {
        let music_tracks = self
            .state_service
            .get_session(self.guild_id, self.channel_id)
            .await?
            .map(|s| {
                s.get_all_track_ids_by_queue_name_prefix("music")
                    .into_iter()
                    .filter_map(|tid| s.find_track(tid).cloned())
                    .collect::<Vec<Track>>()
            })
            .unwrap_or_default();

        if music_tracks.len() < 2 {
            tracing::debug!("No next track available");
            return Ok(());
        }

        let current_track = &music_tracks[0];
        let current_track_id = current_track.track_id;
        let queue_name = current_track.queue_name.clone();
        let next_track_id = music_tracks[1].track_id;

        tracing::info!(track_id = %current_track_id, "Skipping to next track");

        self.mixer.remove_source(current_track_id);
        self.started.lock().await.remove(&current_track_id);
        modify_state_session(
            &self.state_service,
            self.guild_id,
            self.channel_id,
            move |session| {
                session.remove_track(current_track_id);
            },
        )
        .await?;

        metrics::record_track_lifecycle("skip", &normalize_queue_name(&queue_name));

        self.reconcile().await?;

        let this = Arc::clone(self);
        tokio::spawn(async move {
            if let Err(e) = this.preload_if_possible(next_track_id).await {
                tracing::warn!(track_id = %next_track_id, error = %e, "Background preload failed");
            }
        });

        Ok(())
    }

    #[instrument(skip(self), fields(guild_id = %self.guild_id))]
    pub async fn pause_queue(&self, queue_name: QueueName) -> ZakoResult<()> {
        tracing::info!(queue_name = %queue_name, "Pausing queue");
        let track_ids = self
            .state_service
            .get_session(self.guild_id, self.channel_id)
            .await?
            .map(|s| {
                s.queues
                    .get(&queue_name)
                    .map(|tracks| {
                        tracks
                            .iter()
                            .filter(|t| !t.paused)
                            .map(|t| t.track_id)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        for track_id in track_ids {
            self.pause(track_id).await?;
        }
        Ok(())
    }

    #[instrument(skip(self), fields(guild_id = %self.guild_id))]
    pub async fn resume_queue(&self, queue_name: QueueName) -> ZakoResult<()> {
        tracing::info!(queue_name = %queue_name, "Resuming queue");
        let track_ids = self
            .state_service
            .get_session(self.guild_id, self.channel_id)
            .await?
            .map(|s| {
                s.queues
                    .get(&queue_name)
                    .map(|tracks| {
                        tracks
                            .iter()
                            .filter(|t| t.paused)
                            .map(|t| t.track_id)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        for track_id in track_ids {
            self.resume(track_id).await?;
        }
        Ok(())
    }

    #[instrument(skip(self), fields(guild_id = %self.guild_id))]
    pub async fn session_state(&self) -> ZakoResult<Option<SessionState>> {
        self.state_service
            .get_session(self.guild_id, self.channel_id)
            .await
    }

    #[instrument(skip(self), fields(guild_id = %self.guild_id))]
    pub(crate) async fn reconcile(&self) -> ZakoResult<()> {
        let _guard = self.reconcile_guard.lock().await;

        // Normally one pass. A pass that dropped a stale head re-reads instead
        // of playing, so the track behind it gets its turn in this same call;
        // bounded so a long backlog cannot spin here; the rest waits for the
        // next reconcile.
        for _ in 0..=MAX_STALE_SKIP_ROUNDS {
            let session = self
                .state_service
                .get_session(self.guild_id, self.channel_id)
                .await?;

            let Some(session) = session else {
                return Ok(());
            };

            if self.drop_stale_heads(&session).await? {
                continue;
            }

            let active_tracks = session.get_active_tracks();

            let non_paused_ids: Vec<TrackId> = active_tracks
                .iter()
                .filter(|t| !t.paused)
                .map(|t| t.track_id)
                .collect();
            let present = self.mixer.has_sources(non_paused_ids).await;

            for track in active_tracks {
                if track.paused {
                    continue;
                }
                if !present.contains(&track.track_id) {
                    if let Err(e) = self.play_now(track.clone()).await {
                        tracing::warn!(
                            track_id = %track.track_id,
                            queue_name = %track.queue_name,
                            error = %e,
                            "Failed to start playback, removing track from queue"
                        );
                        self.mixer.remove_source(track.track_id);
                        self.started.lock().await.remove(&track.track_id);
                        let tid = track.track_id;
                        let qn = track.queue_name.clone();
                        modify_state_session(
                            &self.state_service,
                            self.guild_id,
                            self.channel_id,
                            move |session| {
                                session.remove_track(tid);
                            },
                        )
                        .await?;
                        metrics::record_track_lifecycle("fail", &normalize_queue_name(&qn));
                    }
                }
            }

            return Ok(());
        }

        tracing::warn!(
            max_age_secs = self.config.queue_max_age.as_secs(),
            rounds = MAX_STALE_SKIP_ROUNDS,
            "Stopped dropping stale tracks at the per-reconcile round limit; the rest is re-checked on the next reconcile"
        );

        Ok(())
    }

    /// Drops the stale heads of this session, returning whether anything was
    /// dropped.
    ///
    /// Only a head can be stale in a way that matters: it is the only track
    /// that could have played and did not. The tracks behind it have not been
    /// denied anything yet (they are waiting their turn), so they are left
    /// alone until they reach the front, which is where the age is judged.
    async fn drop_stale_heads(&self, session: &SessionState) -> ZakoResult<bool> {
        let max_age_secs = self.config.queue_max_age.as_secs();
        if self.config.queue_max_age.is_zero() {
            return Ok(false);
        }

        let now_ms = now_unix_ms();
        let stale: Vec<(Track, u64)> = session
            .get_active_tracks()
            .into_iter()
            .filter(|track| !track.paused)
            .filter_map(|track| {
                stale_age_secs(&track, now_ms, max_age_secs).map(|age| (track, age))
            })
            .collect();

        if stale.is_empty() {
            return Ok(false);
        }

        for (track, age_secs) in &stale {
            tracing::warn!(
                track_id = %track.track_id,
                queue_name = %track.queue_name,
                tap_id = %track.request.tap_id.0,
                discord_user_id = %track.request.discord_user_id,
                age_secs,
                max_age_secs,
                guild_id = %self.guild_id,
                channel_id = %self.channel_id,
                "Dropping stale queued track instead of playing it"
            );
            metrics::record_queue_stale_skipped(&normalize_queue_name(&track.queue_name));
            // A head should never be in the mixer without having played, but a
            // source left behind after we remove the track would keep mixing.
            self.mixer.remove_source(track.track_id);
            self.started.lock().await.remove(&track.track_id);
        }

        let ids: Vec<TrackId> = stale.iter().map(|(track, _)| track.track_id).collect();
        modify_state_session(
            &self.state_service,
            self.guild_id,
            self.channel_id,
            move |session| {
                for id in ids {
                    session.remove_track(id);
                }
            },
        )
        .await?;

        Ok(true)
    }

    /// Stops tracks whose end event never arrived.
    ///
    /// `reconcile` only runs when a command comes in (play/stop/stop_many/
    /// next_music), so nothing in the engine ever re-examines a track that is
    /// already playing. A track that stalls after it starts therefore stays at
    /// the head of its queue forever, and because `get_active_tracks` only
    /// plays the head, every message queued behind it is silently never played.
    /// This sweep is the time-based check that closes that hole, and it runs on
    /// its own clock rather than on those four callers.
    ///
    /// A track is stopped only when it is also making no progress (the mixer
    /// reports it idle), so a long track that is still playing through the
    /// window is left alone.
    pub(crate) async fn watchdog_sweep(&self) -> ZakoResult<()> {
        if self.config.track_watchdog.is_zero() {
            return Ok(());
        }

        let now = Instant::now();
        let overdue = self.overdue_tracks(now).await;
        if overdue.is_empty() {
            return Ok(());
        }

        // One state read per sweep, and only when something is actually past
        // the window: a paused track is not stuck, and a track that is no
        // longer in the session has nothing to stop.
        let session = self
            .state_service
            .get_session(self.guild_id, self.channel_id)
            .await?;

        let Some(session) = session else {
            self.started.lock().await.clear();
            return Ok(());
        };

        {
            let mut started = self.started.lock().await;
            started.retain(|track_id, _| session.find_track(*track_id).is_some());
        }

        let overdue: Vec<(TrackId, StartedTrack)> = self
            .overdue_tracks(now)
            .await
            .into_iter()
            .filter(|(track_id, _)| {
                session
                    .find_track(*track_id)
                    .map(|track| !track.paused)
                    .unwrap_or(false)
            })
            .collect();

        if overdue.is_empty() {
            return Ok(());
        }

        let track_ids: Vec<TrackId> = overdue.iter().map(|(track_id, _)| *track_id).collect();
        let idle = self
            .mixer
            .idle_sources(track_ids, self.config.track_watchdog)
            .await;

        for (track_id, started) in overdue {
            if !idle.contains(&track_id) {
                tracing::debug!(
                    track_id = %track_id,
                    elapsed_secs = now.duration_since(started.started_at).as_secs(),
                    "Track is past the watchdog window but still mixing; leaving it alone"
                );
                continue;
            }

            metrics::record_track_watchdog_fired();
            tracing::warn!(
                track_id = %track_id,
                elapsed_secs = now.duration_since(started.started_at).as_secs(),
                watchdog_secs = self.config.track_watchdog.as_secs(),
                idle_secs = self.config.track_watchdog.as_secs(),
                queue_name = %started.queue_name,
                tap_id = %started.tap_id,
                discord_user_id = %started.discord_user_id.0,
                guild_id = %self.guild_id,
                channel_id = %self.channel_id,
                "Track watchdog fired: no end event and no audio progress within the window; stopping the track so the queue can move on"
            );

            // stop() removes the track from the session and reconciles, which
            // is what starts the next track in the queue.
            if let Err(e) = self.stop(track_id).await {
                tracing::warn!(
                    track_id = %track_id,
                    error = %e,
                    "Track watchdog failed to stop the stalled track"
                );
            }
        }

        Ok(())
    }

    /// Started tracks whose watchdog window has passed, newest bookkeeping read
    /// under the lock and returned.
    async fn overdue_tracks(&self, now: Instant) -> Vec<(TrackId, StartedTrack)> {
        let started = self.started.lock().await;
        started
            .iter()
            .filter(|(_, s)| now.duration_since(s.started_at) >= self.config.track_watchdog)
            .map(|(track_id, s)| (*track_id, s.clone()))
            .collect()
    }

    #[instrument(skip(self), fields(guild_id = %self.guild_id))]
    async fn play_now(&self, track: Track) -> ZakoResult<()> {
        tracing::info!(
            track_id = %track.track_id,
            queue_name = %track.queue_name,
            "Starting playback"
        );

        let response = self
            .taphub_service
            .request_audio(track.request.clone())
            .await?;

        let consumer = self
            .decoder
            .start_decoding(track.track_id, response.stream)
            .await?;

        self.mixer
            .add_source(track.track_id, consumer, self.end_tx.clone());
        self.mixer.set_volume(track.track_id, track.volume.into());

        // The watchdog's clock starts when the mixer is told to play: after
        // that, the only thing that can end the track is an end event from the
        // mixer (or a stall, which is what the watchdog is for).
        self.started.lock().await.insert(
            track.track_id,
            StartedTrack {
                started_at: Instant::now(),
                queue_name: track.queue_name.clone(),
                tap_id: track.request.tap_id.clone(),
                discord_user_id: track.request.discord_user_id.clone(),
            },
        );

        metrics::record_track_lifecycle("start", &normalize_queue_name(&track.queue_name));

        Ok(())
    }

    #[instrument(skip(self), fields(guild_id = %self.guild_id))]
    async fn handle_ended_track(&self, track_id: TrackId) -> ZakoResult<()> {
        tracing::info!(track_id = %track_id, "Track ended naturally");

        let queue_name = self
            .state_service
            .get_session(self.guild_id, self.channel_id)
            .await?
            .and_then(|s| s.find_track(track_id).map(|t| t.queue_name.clone()));

        if let Some(qn) = &queue_name {
            metrics::record_track_lifecycle("end", &normalize_queue_name(qn));
        }

        self.stop(track_id).await?;

        self.preload_if_possible(track_id).await?;

        Ok(())
    }

    #[instrument(skip(self), fields(guild_id = %self.guild_id))]
    async fn preload_if_possible(&self, track_id: TrackId) -> ZakoResult<()> {
        let session = self
            .state_service
            .get_session(self.guild_id, self.channel_id)
            .await?;

        if let Some(session) = session {
            let queue_name = match session.find_track(track_id) {
                Some(track) => track.queue_name.clone(),
                None => {
                    metrics::record_preload("miss");
                    return Ok(());
                }
            };

            let track_ids = session.get_all_track_ids_by_queue_name(&queue_name);
            if track_ids.len() >= 2 {
                let next_track_id = track_ids[1];
                let next_track = session.find_track(next_track_id);
                if let Some(track) = next_track {
                    tracing::debug!(next_track_id = %next_track_id, "Preloading next track");
                    match self
                        .taphub_service
                        .preload_audio(track.request.clone())
                        .await
                    {
                        Ok(_) => {
                            metrics::record_preload("hit");
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Preload failed");
                            metrics::record_preload("error");
                        }
                    }
                    return Ok(());
                }
            }
        }

        metrics::record_preload("miss");
        Ok(())
    }
}

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// Whether a queue is a per-user TTS queue, the only kind the age cap applies
/// to.
///
/// A TTS queue is one user's speech and it expires: a message read a minute
/// late is worse than a message not read. Music queues are playlists: a track
/// sitting there for ten minutes is waiting its turn as intended, and dropping
/// it would be a bug, not a fix.
fn is_tts_queue(queue_name: &QueueName) -> bool {
    let name: String = queue_name.clone().into();
    name.starts_with("tts_")
}

/// The age in seconds of a track that is too old to play, or `None` when it
/// should play. Pure, so the rule is testable without a state service.
pub(crate) fn stale_age_secs(track: &Track, now_ms: i64, max_age_secs: u64) -> Option<u64> {
    if max_age_secs == 0 || !is_tts_queue(&track.queue_name) {
        return None;
    }

    // No enqueue time means the age is unknown: state written before the field
    // existed, or a track restored from such state. Playing it is the safe
    // direction: silently dropping audio we cannot date loses messages.
    let queued_at_ms = track.queued_at_ms?;
    let age_ms = now_ms.checked_sub(queued_at_ms)?;
    if age_ms < 0 {
        // The wall clock stepped backwards; we cannot claim the track is old.
        return None;
    }

    let age_secs = (age_ms / 1000) as u64;
    (age_secs > max_age_secs).then_some(age_secs)
}

fn normalize_queue_name(queue_name: &QueueName) -> String {
    let qn: String = queue_name.clone().into();
    if qn.starts_with("tts_") {
        "tts".to_string()
    } else if qn.starts_with("music") {
        "music".to_string()
    } else {
        "other".to_string()
    }
}

fn upsert_track(queues: &mut HashMap<QueueName, Vec<Track>>, queue_name: QueueName, track: Track) {
    if let Some(queue) = queues.get_mut(&queue_name) {
        queue.push(track);
    } else {
        queues.insert(queue_name.clone(), vec![track]);
    }
}

pub fn create_session_control(
    guild_id: GuildId,
    channel_id: ChannelId,
    mixer: ArcMixer,
    decoder: ArcDecoder,
    state_service: ArcStateService,
    taphub_service: ArcTapHubService,
    config: SessionConfig,
    mixer_alive: Arc<AtomicBool>,
) -> Arc<SessionControl> {
    let (end_tx, end_rx) = tokio::sync::mpsc::channel(16);

    let session_control = Arc::new(SessionControl::new(
        guild_id,
        channel_id,
        end_tx,
        mixer,
        decoder,
        state_service,
        taphub_service,
        config,
        mixer_alive,
    ));

    let sc_clone = session_control.clone();
    tokio::spawn(async move {
        let mut end_rx = end_rx;
        // Stall-free track-end handling: each ended track is handled in its own
        // spawned task so a slow TTS/preload on one track can never block the
        // handling of subsequent ended tracks (which previously serialized the
        // whole loop behind a taphub call). Per-track work is safe to run
        // concurrently: distinct track_ids touch distinct mixer/decoder slots,
        // and any shared reconcile is still serialized by reconcile_guard.
        while let Some(track_id) = end_rx.recv().await {
            let sc = sc_clone.clone();
            tokio::spawn(async move {
                if let Err(e) = sc.handle_ended_track(track_id).await {
                    tracing::warn!(track_id = %track_id, error = %e, "Failed to handle ended track");
                }
            });
        }
    });

    spawn_track_watchdog(Arc::downgrade(&session_control), config.track_watchdog);

    session_control
}

/// Runs `watchdog_sweep` on its own clock for as long as the session lives.
///
/// The driver holds a `Weak` so the watchdog never keeps a dropped session (and
/// its mixer thread) alive, and the tick scales with the window so a short
/// window is checked often enough to fire inside it.
fn spawn_track_watchdog(session: Weak<SessionControl>, watchdog: Duration) {
    if watchdog.is_zero() {
        return;
    }

    let tick = (watchdog / 10).clamp(Duration::from_millis(200), Duration::from_secs(1));

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tick).await;

            let Some(control) = session.upgrade() else {
                break;
            };

            if let Err(e) = control.watchdog_sweep().await {
                tracing::warn!(error = %e, "Track watchdog sweep failed");
            }
        }
    });
}
