// warn: AI generated
//
// The session write lock (`state.rs`) exists because `modify_state_session`
// used to be a lock-free read-modify-write: two concurrent `play`s both read
// the session, each added one track, and the second `save_session` overwrote
// the first — the queued track vanished with no log line anywhere. These tests
// are the proof that the loss is gone and that the lock cannot deadlock
// against `SessionControl::reconcile_guard`.

use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use zako3_audio_engine_audio::{MockDecoder, MockMixer, create_ringbuf_pair};
use zako3_types::hq::{DiscordUserId, TapId};

use super::{StateService, modify_state_session, with_state_session};
use crate::{
    engine::session::create_session_control,
    error::ZakoResult,
    service::{state::ArcStateService, taphub::MockTapHubService},
    types::{
        AudioCachePolicy, AudioCacheType, AudioMetaResponse, AudioMetadata, AudioRequestString,
        AudioResponse, CachedAudioRequest, ChannelId, GuildId, QueueName, SessionState, Track,
        TrackId, Volume,
    },
};

/// A `StateService` holding one session, with an artificial delay between the
/// read and the write. Without the lock this delay is what makes two writers
/// interleave and lose an update, every time.
struct SlowStateService {
    session: StdMutex<Option<SessionState>>,
    delay: Duration,
    saves: AtomicUsize,
}

impl SlowStateService {
    fn new(session: SessionState, delay: Duration) -> Self {
        Self {
            session: StdMutex::new(Some(session)),
            delay,
            saves: AtomicUsize::new(0),
        }
    }

    fn snapshot(&self) -> SessionState {
        self.session
            .lock()
            .unwrap()
            .clone()
            .expect("session should still exist")
    }
}

#[async_trait]
impl StateService for SlowStateService {
    async fn get_session(
        &self,
        _guild_id: GuildId,
        _channel_id: ChannelId,
    ) -> ZakoResult<Option<SessionState>> {
        tokio::time::sleep(self.delay).await;
        Ok(self.session.lock().unwrap().clone())
    }

    async fn save_session(&self, session: &SessionState) -> ZakoResult<()> {
        tokio::time::sleep(self.delay).await;
        self.saves.fetch_add(1, Ordering::Relaxed);
        *self.session.lock().unwrap() = Some(session.clone());
        Ok(())
    }

    async fn delete_session(
        &self,
        _guild_id: GuildId,
        _channel_id: ChannelId,
    ) -> ZakoResult<()> {
        *self.session.lock().unwrap() = None;
        Ok(())
    }

    async fn list_sessions(&self) -> ZakoResult<Vec<SessionState>> {
        Ok(self
            .session
            .lock()
            .unwrap()
            .clone()
            .into_iter()
            .collect::<Vec<_>>())
    }

    async fn list_sessions_in_guild(
        &self,
        _guild_id: GuildId,
    ) -> ZakoResult<Vec<SessionState>> {
        self.list_sessions().await
    }
}

fn empty_session(guild_id: GuildId, channel_id: ChannelId) -> SessionState {
    SessionState {
        guild_id,
        channel_id,
        queues: Default::default(),
    }
}

fn dummy_track(id: u64) -> Track {
    Track {
        track_id: TrackId::from(id),
        metadatas: vec![AudioMetadata::Title(format!("track {id}"))],
        request: CachedAudioRequest {
            tap_id: TapId("test_tap".to_string()),
            audio_request: AudioRequestString::from("req".to_string()),
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
            discord_user_id: DiscordUserId::from("123".to_string()),
            headers: Default::default(),
        },
        volume: Volume::from(1.0),
        queue_name: QueueName::from("music".to_string()),
        paused: false,
    }
}

fn queued_track_ids(session: &SessionState) -> HashSet<u64> {
    session
        .queues
        .get(&QueueName::from("music".to_string()))
        .map(|tracks| tracks.iter().map(|t| u64::from(t.track_id)).collect())
        .unwrap_or_default()
}

/// The regression test for the lost update: N writers each queue one track on
/// the same session, concurrently, and every single one has to survive.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_writers_keep_every_update() {
    const N: u64 = 32;

    let guild_id = GuildId::from(9_001);
    let channel_id = ChannelId::from(9_002);
    let service = Arc::new(SlowStateService::new(
        empty_session(guild_id, channel_id),
        Duration::from_millis(2),
    ));
    let state: ArcStateService = service.clone();

    let mut tasks = Vec::new();
    for id in 1..=N {
        let state = state.clone();
        tasks.push(tokio::spawn(async move {
            modify_state_session(&state, guild_id, channel_id, move |session| {
                session
                    .queues
                    .entry(QueueName::from("music".to_string()))
                    .or_default()
                    .push(dummy_track(id));
            })
            .await
        }));
    }

    for task in tasks {
        task.await.expect("writer task panicked").expect("write failed");
    }

    let queued = queued_track_ids(&service.snapshot());
    assert_eq!(
        queued.len() as u64,
        N,
        "queue lost entries: expected {N}, got {} ({queued:?})",
        queued.len()
    );
    assert_eq!(queued, (1..=N).collect::<HashSet<_>>());
}

/// A writer that finds no session must not create one and must not call the
/// closure — the missing-session case stays an explicit decision elsewhere.
#[tokio::test]
async fn missing_session_is_left_alone() {
    let guild_id = GuildId::from(9_010);
    let channel_id = ChannelId::from(9_011);
    let mut state = SlowStateService::new(empty_session(guild_id, channel_id), Duration::ZERO);
    state.session = StdMutex::new(None);
    let state: ArcStateService = Arc::new(state);

    let called = Arc::new(AtomicUsize::new(0));
    let called_in_closure = called.clone();
    let changed = with_state_session(&state, guild_id, channel_id, move |_| {
        called_in_closure.fetch_add(1, Ordering::Relaxed);
    })
    .await
    .expect("no error");

    assert!(changed.is_none(), "no session means no result");
    assert_eq!(called.load(Ordering::Relaxed), 0);
}

/// Exercises the documented lock order end to end: `play` writes the session
/// (session lock, released), then reconciles (reconcile_guard, then session
/// lock), while other tasks are doing the same and stopping tracks. If the
/// order were inverted anywhere this would hang instead of finishing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_play_and_stop_do_not_deadlock() {
    const PLAYS: u64 = 24;

    let guild_id = GuildId::from(9_020);
    let channel_id = ChannelId::from(9_021);
    let service = Arc::new(SlowStateService::new(
        empty_session(guild_id, channel_id),
        Duration::from_millis(1),
    ));
    let state: ArcStateService = service.clone();

    let mut mixer = MockMixer::new();
    // Nothing is playing, so every reconcile wants to start the head track.
    mixer.expect_has_sources().returning(|_| HashSet::new());
    mixer.expect_add_source().return_const(());
    mixer.expect_set_volume().return_const(());
    mixer.expect_remove_source().return_const(());

    let mut decoder = MockDecoder::new();
    decoder.expect_start_decoding().returning(|_, _| {
        let (_, consumer) = create_ringbuf_pair();
        Ok(consumer)
    });
    decoder.expect_stop_track().return_const(());

    let mut taphub = MockTapHubService::new();
    taphub.expect_request_audio_meta().returning(|_| {
        Ok(AudioMetaResponse {
            metadatas: vec![AudioMetadata::Title("t".to_string())],
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
            base_volume: 1.0,
        })
    });
    taphub.expect_request_audio().returning(|_| {
        Ok(AudioResponse {
            metadatas: vec![AudioMetadata::Title("t".to_string())],
            cache_key: None,
            stream: tokio::sync::mpsc::channel(1).1,
        })
    });
    taphub.expect_preload_audio().returning(|_| {
        Ok(AudioMetaResponse {
            metadatas: vec![],
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
            base_volume: 1.0,
        })
    });

    let control = create_session_control(
        guild_id,
        ChannelId::from(100),
        Arc::new(mixer),
        Arc::new(decoder),
        state.clone(),
        Arc::new(taphub),
    );

    let run = async {
        let mut tasks = Vec::new();
        for id in 1..=PLAYS {
            let play_control = control.clone();
            tasks.push(tokio::spawn(async move {
                let _ = play_control
                    .play(
                        QueueName::from("music".to_string()),
                        TapId("yt".to_string()),
                        AudioRequestString::from(format!("req{id}")),
                        Volume::from(1.0),
                        DiscordUserId::from("123".to_string()),
                    )
                    .await;
            }));
            // interleave stops: each one takes reconcile_guard then the lock
            let stop_control = control.clone();
            tasks.push(tokio::spawn(async move {
                let _ = stop_control.stop(TrackId::from(id)).await;
            }));
        }
        for task in tasks {
            let _ = task.await;
        }
    };

    tokio::time::timeout(Duration::from_secs(20), run)
        .await
        .expect("play/stop deadlocked: lock order between reconcile_guard and the session lock is inverted");
}

/// The acceptance test in the words of the audit: N simultaneous `play` calls
/// on one session leave exactly N tracks queued.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn n_concurrent_plays_queue_exactly_n_tracks() {
    const N: u64 = 16;

    let guild_id = GuildId::from(9_030);
    let channel_id = ChannelId::from(9_031);
    let service = Arc::new(SlowStateService::new(
        empty_session(guild_id, channel_id),
        Duration::from_millis(2),
    ));
    let state: ArcStateService = service.clone();

    let mut mixer = MockMixer::new();
    // Report every track as already playing, so reconcile never removes a
    // track: the queue this test measures is only what `play` queued.
    mixer.expect_has_sources().returning(|ids| ids.into_iter().collect());
    mixer.expect_add_source().return_const(());
    mixer.expect_set_volume().return_const(());
    mixer.expect_remove_source().return_const(());

    let mut decoder = MockDecoder::new();
    decoder.expect_start_decoding().returning(|_, _| {
        let (_, consumer) = create_ringbuf_pair();
        Ok(consumer)
    });
    decoder.expect_stop_track().return_const(());

    let mut taphub = MockTapHubService::new();
    taphub.expect_request_audio_meta().returning(|_| {
        Ok(AudioMetaResponse {
            metadatas: vec![AudioMetadata::Title("t".to_string())],
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
            base_volume: 1.0,
        })
    });
    taphub.expect_request_audio().returning(|_| {
        Ok(AudioResponse {
            metadatas: vec![AudioMetadata::Title("t".to_string())],
            cache_key: None,
            stream: tokio::sync::mpsc::channel(1).1,
        })
    });
    taphub.expect_preload_audio().returning(|_| {
        Ok(AudioMetaResponse {
            metadatas: vec![],
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::None,
                ttl_seconds: None,
            },
            base_volume: 1.0,
        })
    });

    let control = create_session_control(
        guild_id,
        ChannelId::from(100),
        Arc::new(mixer),
        Arc::new(decoder),
        state.clone(),
        Arc::new(taphub),
    );

    let mut tasks = Vec::new();
    for id in 1..=N {
        let control = control.clone();
        tasks.push(tokio::spawn(async move {
            control
                .play(
                    QueueName::from("music".to_string()),
                    TapId("yt".to_string()),
                    AudioRequestString::from(format!("req{id}")),
                    Volume::from(1.0),
                    DiscordUserId::from("123".to_string()),
                )
                .await
        }));
    }
    for task in tasks {
        task.await.expect("play task panicked").expect("play failed");
    }

    let queued = queued_track_ids(&service.snapshot());
    assert_eq!(
        queued.len() as u64,
        N,
        "{N} concurrent plays queued only {} tracks",
        queued.len()
    );
    assert_eq!(
        control
            .session_state()
            .await
            .expect("state read ok")
            .expect("session exists")
            .queues
            .values()
            .map(Vec::len)
            .sum::<usize>() as u64,
        N,
        "the visible session must hold exactly {N} tracks"
    );
}
