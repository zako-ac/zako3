// Queue age cap: a TTS track that waited longer than the cap is dropped before
// it is played, the fresh track behind it plays in the same reconcile, and a
// fresh queue is left untouched.
//
// The rule itself is pure (`stale_age_secs`) and is tested directly. The skip
// path is then exercised through `reconcile` with a mocked state service, so
// what is proven is the behaviour the audit asked for: the stale head leaves
// the queue, its removal is saved, and the next track starts.

use std::collections::HashMap;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Duration;

use mockall::predicate::*;
use zako3_audio_engine_audio::{MockDecoder, MockMixer, create_ringbuf_pair};

use crate::engine::session::{
    DEFAULT_QUEUE_MAX_AGE_SECS, SessionConfig, create_session_control, stale_age_secs,
};
use crate::service::{state::MockStateService, taphub::MockTapHubService};
use crate::types::{
    AudioCachePolicy, AudioCacheType, AudioMetadata, AudioRequestString, AudioResponse,
    CachedAudioRequest, ChannelId, GuildId, QueueName, SessionState, Track, TrackId, Volume,
};
use zako3_types::hq::{DiscordUserId, TapId};

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

fn track(id: u64, queue: &str, queued_at_ms: Option<i64>) -> Track {
    Track {
        track_id: TrackId::from(id),
        metadatas: vec![AudioMetadata::Title(format!("T{id}"))],
        request: CachedAudioRequest {
            tap_id: TapId("tts_tap".to_string()),
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
        queued_at_ms,
    }
}

fn session_with(guild_id: GuildId, queue: &str, tracks: Vec<Track>) -> SessionState {
    let mut s = SessionState {
        guild_id,
        channel_id: ChannelId::from(100),
        queues: HashMap::new(),
    };
    s.queues.insert(QueueName::from(queue.to_string()), tracks);
    s
}

/// Session config with only the age cap armed: the watchdog is off so these
/// tests have no background task touching the mocks.
fn age_cap(secs: u64) -> SessionConfig {
    SessionConfig {
        track_watchdog: Duration::ZERO,
        queue_max_age: Duration::from_secs(secs),
    }
}

// --- the rule ------------------------------------------------------------

#[test]
fn fresh_tts_is_not_stale() {
    let t = track(1, "tts_123", Some(now_ms() - 1_000));
    assert_eq!(stale_age_secs(&t, now_ms(), 90), None);
}

#[test]
fn tts_past_the_cap_is_stale_and_reports_its_age() {
    let now = now_ms();
    let t = track(1, "tts_123", Some(now - 120_000));
    assert_eq!(stale_age_secs(&t, now, 90), Some(120));
}

#[test]
fn the_cap_is_exclusive_so_exactly_max_age_still_plays() {
    let now = now_ms();
    let t = track(1, "tts_123", Some(now - 90_000));
    assert_eq!(stale_age_secs(&t, now, 90), None);
}

#[test]
fn an_undated_track_plays_rather_than_being_dropped() {
    // State written before `queued_at_ms` existed deserialises to `None`; those
    // tracks cannot be dated, and losing audio is the worse bug.
    let t = track(1, "tts_123", None);
    assert_eq!(stale_age_secs(&t, now_ms(), 90), None);
}

#[test]
fn music_is_never_stale() {
    // A playlist track waiting its turn is the intended behaviour, not a stall.
    let t = track(1, "music", Some(now_ms() - 3_600_000));
    assert_eq!(stale_age_secs(&t, now_ms(), 90), None);
}

#[test]
fn zero_disables_the_cap() {
    let t = track(1, "tts_123", Some(now_ms() - 3_600_000));
    assert_eq!(stale_age_secs(&t, now_ms(), 0), None);
}

#[test]
fn a_clock_that_stepped_backwards_does_not_drop_audio() {
    let now = now_ms();
    let t = track(1, "tts_123", Some(now + 5_000));
    assert_eq!(stale_age_secs(&t, now, 90), None);
}

#[test]
fn the_default_cap_is_ninety_seconds() {
    assert_eq!(DEFAULT_QUEUE_MAX_AGE_SECS, 90);
    assert_eq!(
        SessionConfig::default().queue_max_age,
        Duration::from_secs(90)
    );
}

// --- the skip path -------------------------------------------------------

/// A queue whose head is past the cap and whose second track is fresh: the head
/// is dropped (and the removal saved), then the fresh track plays.
#[tokio::test]
async fn reconcile_drops_the_stale_head_and_plays_the_next_track() {
    let guild_id = GuildId::from(7);
    let mut mock_mixer = MockMixer::new();
    let mut mock_decoder = MockDecoder::new();
    let mut mock_state = MockStateService::new();
    let mut mock_taphub = MockTapHubService::new();

    let stale = track(11, "tts_123", Some(now_ms() - 300_000));
    let fresh = track(12, "tts_123", Some(now_ms() - 500));

    // Read 1: reconcile sees the stale head and drops it.
    let with_stale = session_with(guild_id, "tts_123", vec![stale.clone(), fresh.clone()]);
    // Read 2: modify_state_session's read-modify-write of the removal.
    let before_removal = session_with(guild_id, "tts_123", vec![stale.clone(), fresh.clone()]);
    // Read 3: the re-read that plays what is left.
    let after_removal = session_with(guild_id, "tts_123", vec![fresh.clone()]);

    let reads = std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::from(
        vec![with_stale, before_removal, after_removal],
    )));
    let reads_for_mock = reads.clone();
    mock_state
        .expect_get_session()
        .times(3)
        .returning(move |_, _| Ok(reads_for_mock.lock().unwrap().pop_front()));

    // The removal is persisted before the re-read, and only the fresh track is
    // left in the saved session.
    mock_state
        .expect_save_session()
        .times(1)
        .returning(|s: &SessionState| {
            let queue = s.queues.get(&QueueName::from("tts_123".to_string()));
            assert_eq!(
                queue.map(|q| q.len()),
                Some(1),
                "the stale head must be gone from the saved session"
            );
            assert_eq!(queue.unwrap()[0].track_id, TrackId::from(12));
            Ok(())
        });

    mock_mixer
        .expect_has_sources()
        .times(1)
        .returning(|_| std::collections::HashSet::new());
    // The stale head is cleared from the mixer as a guard; the fresh track is
    // never removed.
    mock_mixer
        .expect_remove_source()
        .with(eq(TrackId::from(11)))
        .times(1)
        .return_const(());

    mock_taphub.expect_request_audio().times(1).returning(|_| {
        Ok(AudioResponse {
            metadatas: vec![AudioMetadata::Title("T12".to_string())],
            cache_key: Some(AudioCachePolicy {
                cache_type: AudioCacheType::CacheKey("key".to_string()),
                ttl_seconds: None,
            }),
            stream: tokio::sync::mpsc::channel(1).1,
        })
    });
    mock_decoder
        .expect_start_decoding()
        .times(1)
        .returning(|_, _| {
            let (_, c) = create_ringbuf_pair();
            Ok(c)
        });
    mock_mixer.expect_add_source().times(1).return_const(());
    mock_mixer.expect_set_volume().times(1).return_const(());

    let control = create_session_control(
        guild_id,
        ChannelId::from(100),
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
        age_cap(90),
        Arc::new(AtomicBool::new(true)),
    );

    control.reconcile().await.expect("reconcile");
    assert!(
        reads.lock().unwrap().is_empty(),
        "reconcile should have consumed exactly the three scripted reads"
    );
}

/// A fresh queue is left exactly as it is: no removal, no write, and the head
/// plays.
#[tokio::test]
async fn reconcile_leaves_a_fresh_track_alone() {
    let guild_id = GuildId::from(8);
    let mut mock_mixer = MockMixer::new();
    let mut mock_decoder = MockDecoder::new();
    let mut mock_state = MockStateService::new();
    let mut mock_taphub = MockTapHubService::new();

    let fresh = track(21, "tts_123", Some(now_ms() - 2_000));
    mock_state
        .expect_get_session()
        .times(1)
        .returning(move |_, _| Ok(Some(session_with(guild_id, "tts_123", vec![fresh.clone()]))));
    // `save_session` has no expectation: any write fails the test.

    mock_mixer
        .expect_has_sources()
        .times(1)
        .returning(|_| std::collections::HashSet::new());
    mock_mixer.expect_remove_source().times(0);

    mock_taphub.expect_request_audio().times(1).returning(|_| {
        Ok(AudioResponse {
            metadatas: vec![AudioMetadata::Title("T21".to_string())],
            cache_key: Some(AudioCachePolicy {
                cache_type: AudioCacheType::CacheKey("key".to_string()),
                ttl_seconds: None,
            }),
            stream: tokio::sync::mpsc::channel(1).1,
        })
    });
    mock_decoder
        .expect_start_decoding()
        .times(1)
        .returning(|_, _| {
            let (_, c) = create_ringbuf_pair();
            Ok(c)
        });
    mock_mixer.expect_add_source().times(1).return_const(());
    mock_mixer.expect_set_volume().times(1).return_const(());

    let control = create_session_control(
        guild_id,
        ChannelId::from(100),
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
        age_cap(90),
        Arc::new(AtomicBool::new(true)),
    );

    control.reconcile().await.expect("reconcile");
}

/// A track that is already playing is not dropped when it passes the cap.
///
/// The cap is about the *wait*: a long TTS that is being heard must not be cut
/// mid-sentence because its enqueue time is now long ago. `reconcile` only
/// judges tracks that have not started, so the second pass here — the same
/// track, re-dated far in the past, still in the mixer — must change nothing.
#[tokio::test]
async fn a_track_already_playing_is_not_dropped_when_it_passes_the_cap() {
    let guild_id = GuildId::from(9);
    let mut mock_mixer = MockMixer::new();
    let mut mock_decoder = MockDecoder::new();
    let mut mock_state = MockStateService::new();
    let mut mock_taphub = MockTapHubService::new();

    let fresh = track(31, "tts_123", Some(now_ms() - 1_000));
    let aged = track(31, "tts_123", Some(now_ms() - 600_000));

    let reads = std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::from(
        vec![
            session_with(guild_id, "tts_123", vec![fresh]),
            session_with(guild_id, "tts_123", vec![aged]),
        ],
    )));
    let reads_for_mock = reads.clone();
    mock_state
        .expect_get_session()
        .times(2)
        .returning(move |_, _| Ok(reads_for_mock.lock().unwrap().pop_front()));
    // `save_session` has no expectation: a drop would fail the test.

    let mixer_rounds = std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::from(
        vec![
            std::collections::HashSet::new(),
            std::collections::HashSet::from([TrackId::from(31)]),
        ],
    )));
    mock_mixer
        .expect_has_sources()
        .times(2)
        .returning(move |_| mixer_rounds.lock().unwrap().pop_front().unwrap());
    mock_mixer.expect_remove_source().times(0);

    mock_taphub.expect_request_audio().times(1).returning(|_| {
        Ok(AudioResponse {
            metadatas: vec![AudioMetadata::Title("T31".to_string())],
            cache_key: Some(AudioCachePolicy {
                cache_type: AudioCacheType::CacheKey("key".to_string()),
                ttl_seconds: None,
            }),
            stream: tokio::sync::mpsc::channel(1).1,
        })
    });
    mock_decoder
        .expect_start_decoding()
        .times(1)
        .returning(|_, _| {
            let (_, c) = create_ringbuf_pair();
            Ok(c)
        });
    mock_mixer.expect_add_source().times(1).return_const(());
    mock_mixer.expect_set_volume().times(1).return_const(());

    let control = create_session_control(
        guild_id,
        ChannelId::from(100),
        Arc::new(mock_mixer),
        Arc::new(mock_decoder),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
        age_cap(90),
        Arc::new(AtomicBool::new(true)),
    );

    // Pass one: fresh, so it starts and enters `started`.
    control.reconcile().await.expect("first reconcile");
    // Pass two: the same track, now past the cap but already playing.
    control.reconcile().await.expect("second reconcile");
    assert!(
        reads.lock().unwrap().is_empty(),
        "both scripted reads should have been consumed"
    );
}
