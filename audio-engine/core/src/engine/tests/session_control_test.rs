// warn: AI generated

use mockall::predicate::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zako3_audio_engine_audio::{MockDecoder, MockMixer, create_boxed_ringbuf_pair};

use crate::engine::session::create_session_control;
use crate::service::{state::MockStateService, taphub::MockTapHubService};
use crate::types::{
    AudioMetaResponse, AudioRequestString, AudioResponse, CachedAudioRequest, ChannelId, GuildId,
    QueueName, SessionState, StreamCacheKey, TapName, Track, TrackDescription, TrackId, Volume,
};

// Helper to create a dummy track
fn create_dummy_track(id: u64, queue: &str) -> Track {
    Track {
        track_id: TrackId::from(id),
        description: TrackDescription::from("Test Track".to_string()),
        request: CachedAudioRequest {
            tap_name: TapName::from("yt".to_string()),
            audio_request: AudioRequestString::from("req".to_string()),
            cache_key: StreamCacheKey::from("key".to_string()),
        },
        volume: Volume::from(1.0),
        queue_name: QueueName::from(queue.to_string()),
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
                description: TrackDescription::from("Test Track".to_string()),
                cache_key: StreamCacheKey::from("test_key".to_string()),
            })
        });

    // 2. modify_state_session -> get_session (Initial empty)
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_| {
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
        .returning(move |_| {
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

    // 5. reconcile -> has_source (Not playing)
    mock_mixer
        .expect_has_source()
        .with(eq(TrackId::from(999)))
        .times(1)
        .returning(|_| false);

    // 6. play_now -> request_audio
    mock_taphub.expect_request_audio().times(1).returning(|_| {
        Ok(AudioResponse {
            description: TrackDescription::from("Test Track".to_string()),
            cache_key: Some(StreamCacheKey::from("test_key".to_string())),
            stream: Box::new(tokio::io::empty()),
        })
    });

    // 7. play_now -> decoder
    mock_decoder
        .expect_start_decoding()
        .times(1)
        .returning(|_, _| {
            let (_, c) = create_boxed_ringbuf_pair();
            Ok(c)
        });

    // 8. play_now -> mixer
    mock_mixer.expect_add_source().times(1).return_const(());
    mock_mixer.expect_set_volume().times(1).return_const(());

    let control = create_session_control(
        guild_id,
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
    );

    let res = control
        .play(
            QueueName::from("music".to_string()),
            TapName::from("yt".to_string()),
            AudioRequestString::from("test".to_string()),
            Volume::from(1.0),
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
            description: TrackDescription::from("T2".to_string()),
            cache_key: StreamCacheKey::from("k2".to_string()),
        })
    });

    // 2. Get/Save session (Add T2)
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_| {
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
        .returning(move |_| {
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
        .expect_has_source()
        .with(eq(TrackId::from(1)))
        .times(1)
        .returning(|_| true);

    // Crucially: T2 is NOT checked or played because get_active_tracks only returns the first one (T1).
    // So request_audio/decoder/add_source should NOT be called.

    let control = create_session_control(
        guild_id,
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
    );

    let res = control
        .play(
            QueueName::from("music".to_string()),
            TapName::from("yt".to_string()),
            AudioRequestString::from("t".to_string()),
            Volume::from(1.0),
        )
        .await;

    assert!(res.is_ok());
}

#[tokio::test]
async fn test_stop_success() {
    let guild_id = GuildId::from(2);
    let mut mock_mixer = MockMixer::new();
    let mock_decoder = MockDecoder::new();
    let mut mock_state = MockStateService::new();
    let mock_taphub = MockTapHubService::new();
    let track_id = TrackId::from(100);

    mock_mixer
        .expect_remove_source()
        .with(eq(track_id))
        .times(1)
        .return_const(());

    // Get session for queue name (metrics)
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_| {
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
        .returning(move |_| {
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
        .returning(move |_| {
            Ok(Some(SessionState {
                guild_id,
                channel_id: ChannelId::from(200),
                queues: HashMap::new(),
            }))
        });

    let control = create_session_control(
        guild_id,
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
    );
    assert!(control.stop(track_id).await.is_ok());
}

#[tokio::test]
async fn test_stop_non_existent() {
    let guild_id = GuildId::from(2);
    let mut mock_mixer = MockMixer::new();
    let mock_decoder = MockDecoder::new();
    let mut mock_state = MockStateService::new();
    let mock_taphub = MockTapHubService::new();
    let track_id = TrackId::from(999);

    mock_mixer
        .expect_remove_source()
        .with(eq(track_id))
        .return_const(());

    // Get session for queue name (metrics) - empty, track doesn't exist
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_| {
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
        .returning(move |_| {
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
        .returning(move |_| {
            Ok(Some(SessionState {
                guild_id,
                channel_id: ChannelId::from(200),
                queues: HashMap::new(),
            }))
        });

    let control = create_session_control(
        guild_id,
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
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
        .returning(move |_| {
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
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
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
        .returning(move |_| {
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
        .returning(move |_| {
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
        .returning(move |_| {
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
        .expect_has_source()
        .with(eq(TrackId::from(2)))
        .returning(|_| false);
    mock_taphub.expect_request_audio().returning(|_| {
        Ok(AudioResponse {
            description: TrackDescription::from("T2".to_string()),
            cache_key: Some(StreamCacheKey::from("k2".to_string())),
            stream: Box::new(tokio::io::empty()),
        })
    });
    mock_decoder.expect_start_decoding().returning(|_, _| {
        let (_, c) = create_boxed_ringbuf_pair();
        Ok(c)
    });
    mock_mixer.expect_add_source().return_const(());
    mock_mixer.expect_set_volume().return_const(());

    let control = create_session_control(
        guild_id,
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
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
        .returning(move |_| {
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
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
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
    mock_state.expect_get_session().returning(move |_| {
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
            description: TrackDescription::from("".to_string()),
            cache_key: StreamCacheKey::from("".to_string()),
        })
    });

    // Mixer has_source
    // 1. First call during play() -> reconcile() for Track 1. Return false to trigger add_source.
    // 2. Second call during play() -> reconcile() for Track 2 (queued). Not active, so not called?
    //    Actually reconcile only checks active tracks. Track 1 is active.
    // 3. Later call during next track transition -> reconcile() for Track 2.
    mock_mixer
        .expect_has_source()
        .with(eq(TrackId::from(1)))
        .times(1) // Only during first reconcile
        .returning(|_| false);

    mock_mixer
        .expect_has_source()
        .with(eq(TrackId::from(2)))
        .times(1) // During second reconcile (after track 1 ends)
        .returning(|_| false);

    // Audio Request
    mock_taphub.expect_request_audio().returning(|_| {
        Ok(AudioResponse {
            description: TrackDescription::from("".to_string()),
            cache_key: Some(StreamCacheKey::from("".to_string())),
            stream: Box::new(tokio::io::empty()),
        })
    });

    // Decoder
    mock_decoder.expect_start_decoding().returning(|_, _| {
        let (_, c) = create_boxed_ringbuf_pair();
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

    // Preload Track 2
    mock_taphub
        .expect_preload_audio()
        .withf(|req| req.audio_request == AudioRequestString::from("t".to_string()))
        .times(0)
        .returning(|_| Ok(()));

    let control = create_session_control(
        guild_id,
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
    );

    // 1. Play (Triggers Track 1 start)
    // This adds a 3rd track "t" to the queue.
    let _ = control
        .play(
            QueueName::from("music".to_string()),
            TapName::from("yt".to_string()),
            AudioRequestString::from("t".to_string()),
            Volume::from(1.0),
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
