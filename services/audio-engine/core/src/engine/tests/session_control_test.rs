// warn: AI generated

use mockall::predicate::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zako3_audio_engine_audio::{MockDecoder, MockMixer, create_ringbuf_pair};

use crate::engine::session::{SessionConfig, create_session_control};
use crate::service::{state::MockStateService, taphub::MockTapHubService};
use crate::types::{
    AudioCachePolicy, AudioCacheType, AudioMetaResponse, AudioMetadata, AudioRequestString,
    AudioResponse, CachedAudioRequest, ChannelId, GuildId, QueueName, SessionState, Track, TrackId,
    Volume,
};
use zako3_types::hq::DiscordUserId;
use zako3_types::hq::TapId;

// Helper to create a dummy track
fn create_dummy_track(id: u64, queue: &str) -> Track {
    Track {
        track_id: TrackId::from(id),
        metadatas: vec![AudioMetadata::Title("Test Track".to_string())],
        request: CachedAudioRequest {
            tap_id: TapId("test_tap_id".to_string()),
            audio_request: AudioRequestString::from("req".to_string()),
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::CacheKey("key".to_string()),
                ttl_seconds: None,
            },
            discord_user_id: DiscordUserId::from("123".to_string()),
            headers: Default::default(),
        },
        volume: Volume::from(1.0),
        queue_name: QueueName::from(queue.to_string()),
        paused: false,
    }
}

#[tokio::test]
async fn test_play_success() {
    let guild_id = GuildId::from(1);
    let mut mock_mixer = MockMixer::new();
    let mut mock_decoder = MockDecoder::new();
    let mut mock_state = MockStateService::new();
    let mut mock_taphub = MockTapHubService::new();

    // 1. request_audio_meta
    mock_taphub
        .expect_request_audio_meta()
        .times(1)
        .returning(|_| {
            Ok(AudioMetaResponse {
                metadatas: vec![AudioMetadata::Title("Test Track".to_string())],
                cache_key: AudioCachePolicy {
                    cache_type: AudioCacheType::CacheKey("test_key".to_string()),
                    ttl_seconds: None,
                },
                base_volume: 1.0,
            })
        });

    // 2. modify_state_session -> get_session (Initial empty)
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| {
            Ok(Some(SessionState {
                guild_id,
                channel_id: ChannelId::from(100),
                queues: HashMap::new(),
            }))
        });

    // 3. modify_state_session -> save_session (Track added)
    mock_state
        .expect_save_session()
        .times(1)
        .returning(|_| Ok(()));

    // 4. reconcile -> get_session (Has track)
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| {
            let mut s = SessionState {
                guild_id,
                channel_id: ChannelId::from(100),
                queues: HashMap::new(),
            };
            s.queues.insert(
                QueueName::from("music".to_string()),
                vec![create_dummy_track(999, "music")],
            );
            Ok(Some(s))
        });

    // 5. reconcile -> has_sources (Not playing)
    mock_mixer
        .expect_has_sources()
        .times(1)
        .returning(|_| HashSet::new());

    // 6. play_now -> request_audio
    mock_taphub.expect_request_audio().times(1).returning(|_| {
        Ok(AudioResponse {
            metadatas: vec![AudioMetadata::Title("Test Track".to_string())],
            cache_key: Some(AudioCachePolicy {
                cache_type: AudioCacheType::CacheKey("test_key".to_string()),
                ttl_seconds: None,
            }),
            stream: tokio::sync::mpsc::channel(1).1,
        })
    });

    // 7. play_now -> decoder
    mock_decoder
        .expect_start_decoding()
        .times(1)
        .returning(|_, _| {
            let (_, c) = create_ringbuf_pair();
            Ok(c)
        });

    // 8. play_now -> mixer
    mock_mixer.expect_add_source().times(1).return_const(());
    mock_mixer.expect_set_volume().times(1).return_const(());

    let control = create_session_control(
        guild_id,
        ChannelId::from(100),
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
        SessionConfig::default(),
    );

    let res = control
        .play(
            QueueName::from("music".to_string()),
            TapId("yt_tap_id".to_string()),
            AudioRequestString::from("test".to_string()),
            Volume::from(1.0),
            DiscordUserId::from("123".to_string()),
        )
        .await;

    assert!(res.is_ok());
}

#[tokio::test]
async fn test_play_queued() {
    let guild_id = GuildId::from(1);
    let mut mock_mixer = MockMixer::new();
    let mock_decoder = MockDecoder::new();
    let mut mock_state = MockStateService::new();
    let mut mock_taphub = MockTapHubService::new();

    // 1. Meta
    mock_taphub.expect_request_audio_meta().returning(|_| {
        Ok(AudioMetaResponse {
            metadatas: vec![AudioMetadata::Title("T2".to_string())],
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::CacheKey("k2".to_string()),
                ttl_seconds: None,
            },
            base_volume: 1.0,
        })
    });

    // 2. Get/Save session (Add T2)
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| {
            Ok(Some(SessionState {
                guild_id,
                channel_id: ChannelId::from(100),
                queues: HashMap::new(), // simplified
            }))
        });
    mock_state
        .expect_save_session()
        .times(1)
        .returning(|_| Ok(()));

    // 3. Reconcile -> Get Session (Has T1, T2)
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| {
            let mut s = SessionState {
                guild_id,
                channel_id: ChannelId::from(100),
                queues: HashMap::new(),
            };
            s.queues.insert(
                QueueName::from("music".to_string()),
                vec![
                    create_dummy_track(1, "music"), // Playing
                    create_dummy_track(2, "music"), // Queued
                ],
            );
            Ok(Some(s))
        });

    // 4. Reconcile -> has_source(T1) -> True
    mock_mixer
        .expect_has_sources()
        .times(1)
        .returning(|_| HashSet::from([TrackId::from(1)]));

    // Crucially: T2 is NOT checked or played because get_active_tracks only returns the first one (T1).
    // So request_audio/decoder/add_source should NOT be called.

    let control = create_session_control(
        guild_id,
        ChannelId::from(100),
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
        SessionConfig::default(),
    );

    let res = control
        .play(
            QueueName::from("music".to_string()),
            TapId("yt_tap_id".to_string()),
            AudioRequestString::from("t".to_string()),
            Volume::from(1.0),
            DiscordUserId::from("123".to_string()),
        )
        .await;

    assert!(res.is_ok());
}

#[tokio::test]
async fn test_stop_success() {
    let guild_id = GuildId::from(2);
    let mut mock_mixer = MockMixer::new();
    let mut mock_decoder = MockDecoder::new();
    let mut mock_state = MockStateService::new();
    let mock_taphub = MockTapHubService::new();
    let track_id = TrackId::from(100);

    mock_mixer
        .expect_remove_source()
        .with(eq(track_id))
        .times(1)
        .return_const(());

    mock_decoder
        .expect_stop_track()
        .with(eq(track_id))
        .times(1)
        .return_const(());

    // Get session for queue name (metrics)
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| {
            let mut s = SessionState {
                guild_id,
                channel_id: ChannelId::from(200),
                queues: HashMap::new(),
            };
            s.queues.insert(
                QueueName::from("music".to_string()),
                vec![create_dummy_track(100, "music")],
            );
            Ok(Some(s))
        });
    // Get session (has track)
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| {
            let mut s = SessionState {
                guild_id,
                channel_id: ChannelId::from(200),
                queues: HashMap::new(),
            };
            s.queues.insert(
                QueueName::from("music".to_string()),
                vec![create_dummy_track(100, "music")],
            );
            Ok(Some(s))
        });
    // Save session
    mock_state
        .expect_save_session()
        .times(1)
        .returning(|_| Ok(()));
    // Reconcile -> Get session (empty/removed)
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| {
            Ok(Some(SessionState {
                guild_id,
                channel_id: ChannelId::from(200),
                queues: HashMap::new(),
            }))
        });

    // Reconcile still asks the mixer which tracks are playing, even when the
    // queue is empty.
    mock_mixer
        .expect_has_sources()
        .times(0..)
        .returning(|_| HashSet::new());

    let control = create_session_control(
        guild_id,
        ChannelId::from(100),
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
        SessionConfig::default(),
    );
    assert!(control.stop(track_id).await.is_ok());
}

#[tokio::test]
async fn test_stop_non_existent() {
    let guild_id = GuildId::from(2);
    let mut mock_mixer = MockMixer::new();
    let mut mock_decoder = MockDecoder::new();
    let mut mock_state = MockStateService::new();
    let mock_taphub = MockTapHubService::new();
    let track_id = TrackId::from(999);

    mock_mixer
        .expect_remove_source()
        .with(eq(track_id))
        .return_const(());

    mock_decoder
        .expect_stop_track()
        .with(eq(track_id))
        .times(1)
        .return_const(());

    // Get session for queue name (metrics) - empty, track doesn't exist
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| {
            Ok(Some(SessionState {
                guild_id,
                channel_id: ChannelId::from(200),
                queues: HashMap::new(),
            }))
        });
    // Get session (empty)
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| {
            Ok(Some(SessionState {
                guild_id,
                channel_id: ChannelId::from(200),
                queues: HashMap::new(),
            }))
        });
    // Save session (still empty)
    mock_state
        .expect_save_session()
        .times(1)
        .returning(|_| Ok(()));
    // Reconcile
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| {
            Ok(Some(SessionState {
                guild_id,
                channel_id: ChannelId::from(200),
                queues: HashMap::new(),
            }))
        });

    // Reconcile still asks the mixer which tracks are playing, even when the
    // queue is empty.
    mock_mixer
        .expect_has_sources()
        .times(0..)
        .returning(|_| HashSet::new());

    let control = create_session_control(
        guild_id,
        ChannelId::from(100),
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
        SessionConfig::default(),
    );
    assert!(control.stop(track_id).await.is_ok());
}

#[tokio::test]
async fn test_set_volume() {
    let guild_id = GuildId::from(3);
    let mut mock_mixer = MockMixer::new();
    let mock_decoder = MockDecoder::new();
    let mut mock_state = MockStateService::new();
    let mock_taphub = MockTapHubService::new();
    let track_id = TrackId::from(100);

    mock_mixer
        .expect_set_volume()
        .with(eq(track_id), eq(0.5))
        .times(1)
        .return_const(());

    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| {
            let mut s = SessionState {
                guild_id,
                channel_id: ChannelId::from(300),
                queues: HashMap::new(),
            };
            s.queues.insert(
                QueueName::from("music".to_string()),
                vec![create_dummy_track(100, "music")],
            );
            Ok(Some(s))
        });
    mock_state
        .expect_save_session()
        .times(1)
        .returning(|_| Ok(()));

    let control = create_session_control(
        guild_id,
        ChannelId::from(100),
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
        SessionConfig::default(),
    );
    assert!(
        control
            .set_volume(track_id, Volume::from(0.5))
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn test_next_music_success() {
    let guild_id = GuildId::from(4);
    let mut mock_mixer = MockMixer::new();
    let mut mock_decoder = MockDecoder::new();
    let mut mock_state = MockStateService::new();
    let mut mock_taphub = MockTapHubService::new();

    // 1. Get Session (has 2 tracks)
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| {
            let mut s = SessionState {
                guild_id,
                channel_id: ChannelId::from(400),
                queues: HashMap::new(),
            };
            s.queues.insert(
                QueueName::from("music".to_string()),
                vec![
                    create_dummy_track(1, "music"),
                    create_dummy_track(2, "music"),
                ],
            );
            Ok(Some(s))
        });

    // 2. Remove first
    mock_mixer
        .expect_remove_source()
        .with(eq(TrackId::from(1)))
        .times(1)
        .return_const(());

    // 3. Save (remove 1)
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| {
            let mut s = SessionState {
                guild_id,
                channel_id: ChannelId::from(400),
                queues: HashMap::new(),
            };
            s.queues.insert(
                QueueName::from("music".to_string()),
                vec![
                    create_dummy_track(1, "music"),
                    create_dummy_track(2, "music"),
                ],
            );
            Ok(Some(s))
        });
    mock_state
        .expect_save_session()
        .times(1)
        .returning(|_| Ok(()));

    // 4. Reconcile -> Get Session (has track 2)
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| {
            let mut s = SessionState {
                guild_id,
                channel_id: ChannelId::from(400),
                queues: HashMap::new(),
            };
            s.queues.insert(
                QueueName::from("music".to_string()),
                vec![create_dummy_track(2, "music")],
            );
            Ok(Some(s))
        });

    // 5. Play track 2
    mock_mixer
        .expect_has_sources()
        .returning(|_| HashSet::new());
    mock_taphub.expect_request_audio().returning(|_| {
        Ok(AudioResponse {
            metadatas: vec![AudioMetadata::Title("T2".to_string())],
            cache_key: Some(AudioCachePolicy {
                cache_type: AudioCacheType::CacheKey("k2".to_string()),
                ttl_seconds: None,
            }),
            stream: tokio::sync::mpsc::channel(1).1,
        })
    });
    mock_decoder.expect_start_decoding().returning(|_, _| {
        let (_, c) = create_ringbuf_pair();
        Ok(c)
    });
    mock_mixer.expect_add_source().return_const(());
    mock_mixer.expect_set_volume().return_const(());

    let control = create_session_control(
        guild_id,
        ChannelId::from(100),
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
        SessionConfig::default(),
    );
    assert!(control.next_music().await.is_ok());
}

#[tokio::test]
async fn test_next_music_last_track() {
    let guild_id = GuildId::from(5);
    let mock_mixer = MockMixer::new();
    let mock_decoder = MockDecoder::new();
    let mut mock_state = MockStateService::new();
    let mock_taphub = MockTapHubService::new();

    // 1. Get Session (has 1 track)
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| {
            let mut s = SessionState {
                guild_id,
                channel_id: ChannelId::from(500),
                queues: HashMap::new(),
            };
            s.queues.insert(
                QueueName::from("music".to_string()),
                vec![create_dummy_track(1, "music")],
            );
            Ok(Some(s))
        });

    // Should return early, no remove/save/reconcile

    let control = create_session_control(
        guild_id,
        ChannelId::from(100),
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
        SessionConfig::default(),
    );
    assert!(control.next_music().await.is_ok());
}

#[tokio::test]
async fn test_end_of_track_handling_and_preload() {
    let guild_id = GuildId::from(6);
    let mut mock_mixer = MockMixer::new();
    let mut mock_decoder = MockDecoder::new();
    let mut mock_state = MockStateService::new();
    let mut mock_taphub = MockTapHubService::new();

    // Shared state
    let state_store = Arc::new(Mutex::new(SessionState {
        guild_id,
        channel_id: ChannelId::from(600),
        queues: HashMap::new(),
    }));

    // Initial state: Track 1 playing, Track 2 queued
    {
        let mut s = state_store.lock().unwrap();
        s.queues.insert(
            QueueName::from("music".to_string()),
            vec![
                create_dummy_track(1, "music"),
                create_dummy_track(2, "music"),
            ],
        );
    }

    let end_tx_capture = Arc::new(Mutex::new(None));
    let end_tx_capture_clone = end_tx_capture.clone();

    // Unified get_session mock
    let s_clone = state_store.clone();
    mock_state.expect_get_session().returning(move |_, _| {
        let s = s_clone.lock().unwrap().clone();
        Ok(Some(s))
    });

    // Unified save_session mock
    let s_clone2 = state_store.clone();
    mock_state.expect_save_session().returning(move |s| {
        *s_clone2.lock().unwrap() = s.clone();
        Ok(())
    });

    // Taphub meta (for play call)
    mock_taphub.expect_request_audio_meta().returning(|_| {
        Ok(AudioMetaResponse {
            metadatas: vec![AudioMetadata::Title("".to_string())],
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::CacheKey("".to_string()),
                ttl_seconds: None,
            },
            base_volume: 1.0,
        })
    });

    // Mixer has_source
    // 1. First call during play() -> reconcile() for Track 1. Return false to trigger add_source.
    // 2. Second call during play() -> reconcile() for Track 2 (queued). Not active, so not called?
    //    Actually reconcile only checks active tracks. Track 1 is active.
    // 3. Later call during next track transition -> reconcile() for Track 2.
    mock_mixer
        .expect_has_sources()
        .times(1) // Only during first reconcile
        .returning(|_| HashSet::new());

    mock_mixer
        .expect_has_sources()
        .times(1) // During second reconcile (after track 1 ends)
        .returning(|_| HashSet::new());

    // Audio Request
    mock_taphub.expect_request_audio().returning(|_| {
        Ok(AudioResponse {
            metadatas: vec![AudioMetadata::Title("".to_string())],
            cache_key: Some(AudioCachePolicy {
                cache_type: AudioCacheType::CacheKey("".to_string()),
                ttl_seconds: None,
            }),
            stream: tokio::sync::mpsc::channel(1).1,
        })
    });

    // Decoder
    mock_decoder.expect_start_decoding().returning(|_, _| {
        let (_, c) = create_ringbuf_pair();
        Ok(c)
    });

    // Mixer Add Source - Capture on first call (Track 1)
    mock_mixer
        .expect_add_source()
        .withf(|tid, _, _| *tid == TrackId::from(1))
        .times(1)
        .returning(move |_, _, tx| {
            *end_tx_capture_clone.lock().unwrap() = Some(tx);
        });

    // Mixer Add Source - Second call (Track 2)
    mock_mixer
        .expect_add_source()
        .withf(|tid, _, _| *tid == TrackId::from(2))
        .times(1)
        .return_const(());

    mock_mixer.expect_set_volume().return_const(());

    // --- Expectations for Event ---

    // Remove Track 1
    mock_mixer
        .expect_remove_source()
        .with(eq(TrackId::from(1)))
        .times(1)
        .return_const(());

    mock_decoder
        .expect_stop_track()
        .with(eq(TrackId::from(1)))
        .times(1)
        .return_const(());

    // Preload Track 2
    mock_taphub
        .expect_preload_audio()
        .withf(|req| req.audio_request == AudioRequestString::from("t".to_string()))
        .times(0)
        .returning(|_| {
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
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
        SessionConfig::default(),
    );

    // 1. Play (Triggers Track 1 start)
    // This adds a 3rd track "t" to the queue.
    let _ = control
        .play(
            QueueName::from("music".to_string()),
            TapId("yt_tap_id".to_string()),
            AudioRequestString::from("t".to_string()),
            Volume::from(1.0),
            DiscordUserId::from("123".to_string()),
        )
        .await;

    // 2. Trigger End of Track 1
    let tx = end_tx_capture
        .lock()
        .unwrap()
        .clone()
        .expect("Should have captured tx");
    tx.send(TrackId::from(1)).await.unwrap();

    // 3. Wait for background task
    tokio::time::sleep(Duration::from_millis(100)).await;
}

// ---------------------------------------------------------------------------
// Track watchdog (P0-2)
// ---------------------------------------------------------------------------

use std::collections::HashSet;
use std::sync::atomic::Ordering as AtomicOrdering;
use zako3_audio_engine_audio::metrics::AUDIO_METRICS;

use crate::error::ZakoResult;
use crate::service::state::StateService;

/// A state service that actually stores the session, so a test can follow a
/// track through queue -> start -> watchdog stop -> next track start.
struct FakeState {
    session: tokio::sync::Mutex<Option<SessionState>>,
}

impl FakeState {
    fn new(session: SessionState) -> Self {
        Self {
            session: tokio::sync::Mutex::new(Some(session)),
        }
    }

    async fn snapshot(&self) -> Option<SessionState> {
        self.session.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl StateService for FakeState {
    async fn get_session(
        &self,
        _guild_id: GuildId,
        _channel_id: ChannelId,
    ) -> ZakoResult<Option<SessionState>> {
        Ok(self.session.lock().await.clone())
    }

    async fn save_session(&self, session: &SessionState) -> ZakoResult<()> {
        *self.session.lock().await = Some(session.clone());
        Ok(())
    }

    async fn delete_session(&self, _guild_id: GuildId, _channel_id: ChannelId) -> ZakoResult<()> {
        *self.session.lock().await = None;
        Ok(())
    }

    async fn list_sessions(&self) -> ZakoResult<Vec<SessionState>> {
        Ok(self.session.lock().await.iter().cloned().collect())
    }

    async fn list_sessions_in_guild(&self, _guild_id: GuildId) -> ZakoResult<Vec<SessionState>> {
        Ok(self.session.lock().await.iter().cloned().collect())
    }
}

fn two_track_session(guild_id: GuildId, channel_id: ChannelId) -> SessionState {
    let mut session = SessionState {
        guild_id,
        channel_id,
        queues: HashMap::new(),
    };
    session.queues.insert(
        QueueName::from("tts_1".to_string()),
        vec![
            create_dummy_track(1, "tts_1"),
            create_dummy_track(2, "tts_1"),
        ],
    );
    session
}

fn watchdog_mocks(
    idle: HashSet<TrackId>,
) -> (
    MockMixer,
    MockDecoder,
    MockTapHubService,
    Arc<std::sync::Mutex<Vec<TrackId>>>,
) {
    let started: Arc<std::sync::Mutex<Vec<TrackId>>> = Arc::new(std::sync::Mutex::new(Vec::new()));

    let mut mock_mixer = MockMixer::new();
    mock_mixer
        .expect_has_sources()
        .times(0..)
        .returning(|_| HashSet::new());
    {
        let started = started.clone();
        mock_mixer
            .expect_add_source()
            .times(0..)
            .returning(move |track_id, _consumer, _end_tx| {
                started.lock().unwrap().push(track_id);
            });
    }
    mock_mixer.expect_set_volume().times(0..).return_const(());
    mock_mixer
        .expect_remove_source()
        .times(0..)
        .return_const(());
    mock_mixer
        .expect_idle_sources()
        .times(0..)
        .returning(move |track_ids, _idle| {
            track_ids
                .into_iter()
                .filter(|id| idle.contains(id))
                .collect::<HashSet<TrackId>>()
        });

    let mut mock_decoder = MockDecoder::new();
    mock_decoder
        .expect_start_decoding()
        .times(0..)
        .returning(|_, _| {
            let (_, cons) = create_ringbuf_pair();
            Ok(cons)
        });
    mock_decoder.expect_stop_track().times(0..).return_const(());

    let mut mock_taphub = MockTapHubService::new();
    mock_taphub
        .expect_request_audio_meta()
        .times(0..)
        .returning(|_| {
            Ok(AudioMetaResponse {
                metadatas: vec![],
                cache_key: AudioCachePolicy {
                    cache_type: AudioCacheType::None,
                    ttl_seconds: None,
                },
                base_volume: 1.0,
            })
        });
    mock_taphub
        .expect_request_audio()
        .times(0..)
        .returning(|_| {
            Ok(AudioResponse {
                metadatas: vec![],
                cache_key: None,
                stream: tokio::sync::mpsc::channel(1).1,
            })
        });
    mock_taphub
        .expect_preload_audio()
        .times(0..)
        .returning(|_| {
            Ok(AudioMetaResponse {
                metadatas: vec![],
                cache_key: AudioCachePolicy {
                    cache_type: AudioCacheType::None,
                    ttl_seconds: None,
                },
                base_volume: 1.0,
            })
        });

    (mock_mixer, mock_decoder, mock_taphub, started)
}

/// Serialises the two watchdog tests: both assert on the process-wide
/// `track_watchdog_fired` counter, which the other one would move underneath
/// them.
static WATCHDOG_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A track whose end event never arrives is stopped once the watchdog window
/// passes, and the track queued behind it starts. Without the watchdog this
/// queue is stuck for good: reconcile only runs on a command, so nothing else
/// ever looks at a playing track again.
#[tokio::test]
async fn test_watchdog_stops_stalled_track_and_starts_next() {
    let _guard = WATCHDOG_TEST_LOCK.lock().await;
    let guild_id = GuildId::from(71);
    let channel_id = ChannelId::from(701);

    let before = AUDIO_METRICS
        .track_watchdog_fired
        .load(AtomicOrdering::Relaxed);

    let state = Arc::new(FakeState::new(two_track_session(guild_id, channel_id)));
    let (mock_mixer, mock_decoder, mock_taphub, started) =
        watchdog_mocks(HashSet::from([TrackId::from(1)]));

    let control = create_session_control(
        guild_id,
        channel_id,
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        state.clone(),
        Arc::new(mock_taphub),
        SessionConfig {
            track_watchdog: Duration::from_millis(50),
        },
    );

    // Queueing a message reconciles, which starts the track at the head: 1.
    control
        .play(
            QueueName::from("tts_1".to_string()),
            TapId("test_tap_id".to_string()),
            AudioRequestString::from("stalled".to_string()),
            Volume::from(1.0),
            DiscordUserId::from("123".to_string()),
        )
        .await
        .expect("play should succeed");

    // The mixer reports track 1 as making no progress, and it never sends an
    // end event for it.
    tokio::time::sleep(Duration::from_millis(80)).await;
    control
        .watchdog_sweep()
        .await
        .expect("watchdog sweep should succeed");

    let session = state.snapshot().await.expect("session should still exist");
    let queue = session
        .queues
        .get(&QueueName::from("tts_1".to_string()))
        .expect("queue should survive");

    assert!(
        queue.iter().all(|t| t.track_id != TrackId::from(1)),
        "the stalled track must be removed from the queue"
    );
    assert!(
        queue.iter().any(|t| t.track_id == TrackId::from(2)),
        "the track queued behind it must still be there"
    );

    let started = started.lock().unwrap().clone();
    assert_eq!(
        started.first().copied(),
        Some(TrackId::from(1)),
        "track 1 was the one that started first"
    );
    assert!(
        started.contains(&TrackId::from(2)),
        "after the watchdog stopped track 1 the next track must start, started = {started:?}"
    );

    assert!(
        AUDIO_METRICS
            .track_watchdog_fired
            .load(AtomicOrdering::Relaxed)
            > before,
        "the watchdog counter must move when it fires"
    );
}

/// A track that is still producing audio is not stopped, however far past the
/// window it runs. This is what keeps a long song from being cut at 60s.
#[tokio::test]
async fn test_watchdog_leaves_a_playing_track_alone() {
    let _guard = WATCHDOG_TEST_LOCK.lock().await;
    let guild_id = GuildId::from(72);
    let channel_id = ChannelId::from(702);

    let before = AUDIO_METRICS
        .track_watchdog_fired
        .load(AtomicOrdering::Relaxed);

    let state = Arc::new(FakeState::new(two_track_session(guild_id, channel_id)));
    // The mixer reports nothing as idle: track 1 is playing fine.
    let (mock_mixer, mock_decoder, mock_taphub, started) = watchdog_mocks(HashSet::new());

    let control = create_session_control(
        guild_id,
        channel_id,
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        state.clone(),
        Arc::new(mock_taphub),
        SessionConfig {
            track_watchdog: Duration::from_millis(50),
        },
    );

    control
        .play(
            QueueName::from("tts_1".to_string()),
            TapId("test_tap_id".to_string()),
            AudioRequestString::from("playing".to_string()),
            Volume::from(1.0),
            DiscordUserId::from("123".to_string()),
        )
        .await
        .expect("play should succeed");

    tokio::time::sleep(Duration::from_millis(80)).await;
    control
        .watchdog_sweep()
        .await
        .expect("watchdog sweep should succeed");

    let session = state.snapshot().await.expect("session should still exist");
    let queue = session
        .queues
        .get(&QueueName::from("tts_1".to_string()))
        .expect("queue should survive");

    assert!(
        queue.iter().any(|t| t.track_id == TrackId::from(1)),
        "a track that is still playing must not be stopped by the watchdog"
    );
    assert_eq!(
        started.lock().unwrap().clone(),
        vec![TrackId::from(1)],
        "no second track should have been started"
    );
    assert_eq!(
        AUDIO_METRICS
            .track_watchdog_fired
            .load(AtomicOrdering::Relaxed),
        before,
        "the watchdog must not fire for a playing track"
    );
}
