use std::collections::HashMap;

use crossbeam::channel;
use mockall::predicate::*;

use crate::{
    codec::decoder::MockDecoder,
    engine::{mixer::MockMixer, session::SessionControl},
    service::{MockStateService, MockTapHubService},
    types::{
        AudioRequest, AudioRequestString, AudioStopFilter, GuildId, QueueName, SessionState,
        TapName, Track, TrackId, Volume,
    },
};

#[tokio::test]
async fn test_play() {
    let mut mock_state_service = MockStateService::new();
    let mock_taphub_service = MockTapHubService::new();
    let mock_mixer = MockMixer::new();
    let mock_decoder = MockDecoder::new();

    let (end_tx, _end_rx) = channel::unbounded();

    let guild_id = GuildId::from(1);
    let queue_name = QueueName::from("music".to_string());
    let tap_name = TapName::from("ytdl".to_string());
    let request_str = AudioRequestString::from("https://example.com".to_string());
    let volume = Volume::from(1.0);

    // Mock reconcile -> get_session
    mock_state_service
        .expect_get_session()
        .with(eq(guild_id))
        .returning(|_| {
            Ok(Some(SessionState {
                guild_id: GuildId::from(1),
                channel_id: crate::types::ChannelId::from(1),
                queues: HashMap::new(),
            }))
        });

    // Mock modify_session
    mock_state_service
        .expect_modify_session()
        .with(eq(guild_id), always())
        .returning(|_, _f| Ok(()));

    let session_control = SessionControl {
        guild_id,
        mixer: mock_mixer,
        decoder: mock_decoder,
        end_tx,
        state_service: mock_state_service,
        taphub_service: mock_taphub_service,
    };

    let result = session_control
        .play(queue_name, tap_name, request_str, volume)
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_set_volume() {
    let mut mock_state_service = MockStateService::new();
    let mock_taphub_service = MockTapHubService::new();
    let mut mock_mixer = MockMixer::new();
    let mock_decoder = MockDecoder::new();
    let (end_tx, _end_rx) = channel::unbounded();
    let guild_id = GuildId::from(1);
    let track_id = TrackId::from(123);
    let volume = Volume::from(0.5);

    mock_mixer
        .expect_set_volume()
        .with(eq(track_id), eq(0.5))
        .returning(|_, _| ());

    mock_state_service
        .expect_modify_session()
        .with(eq(guild_id), always())
        .returning(|_, _f| Ok(()));

    let session_control = SessionControl {
        guild_id,
        mixer: mock_mixer,
        decoder: mock_decoder,
        end_tx,
        state_service: mock_state_service,
        taphub_service: mock_taphub_service,
    };

    let result = session_control.set_volume(track_id, volume).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_stop() {
    let mut mock_state_service = MockStateService::new();
    let mock_taphub_service = MockTapHubService::new();
    let mut mock_mixer = MockMixer::new();
    let mock_decoder = MockDecoder::new();
    let (end_tx, _end_rx) = channel::unbounded();
    let guild_id = GuildId::from(1);
    let track_id = TrackId::from(123);

    mock_mixer
        .expect_remove_source()
        .with(eq(track_id))
        .returning(|_| ());

    mock_state_service
        .expect_modify_session()
        .with(eq(guild_id), always())
        .returning(|_, _f| Ok(()));

    let session_control = SessionControl {
        guild_id,
        mixer: mock_mixer,
        decoder: mock_decoder,
        end_tx,
        state_service: mock_state_service,
        taphub_service: mock_taphub_service,
    };

    let result = session_control.stop(track_id).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_stop_many_all() {
    let mut mock_state_service = MockStateService::new();
    let mock_taphub_service = MockTapHubService::new();
    let mut mock_mixer = MockMixer::new();
    let mock_decoder = MockDecoder::new();
    let (end_tx, _end_rx) = channel::unbounded();
    let guild_id = GuildId::from(1);
    let track_id_1 = TrackId::from(101);
    let track_id_2 = TrackId::from(102);

    mock_state_service
        .expect_get_session()
        .with(eq(guild_id))
        .returning(move |_| {
            let mut queues = HashMap::new();
            queues.insert(
                QueueName::from("q1".to_string()),
                vec![Track {
                    track_id: track_id_1,
                    request: AudioRequest {
                        tap_name: TapName::from("t".to_string()),
                        request: AudioRequestString::from("r".to_string()),
                    },
                    volume: Volume::from(1.0),
                    queue_name: QueueName::from("q1".to_string()),
                }],
            );
            queues.insert(
                QueueName::from("q2".to_string()),
                vec![Track {
                    track_id: track_id_2,
                    request: AudioRequest {
                        tap_name: TapName::from("t".to_string()),
                        request: AudioRequestString::from("r".to_string()),
                    },
                    volume: Volume::from(1.0),
                    queue_name: QueueName::from("q2".to_string()),
                }],
            );
            Ok(Some(SessionState {
                guild_id: GuildId::from(1),
                channel_id: crate::types::ChannelId::from(1),
                queues,
            }))
        });

    mock_mixer
        .expect_remove_source()
        .with(eq(track_id_1))
        .returning(|_| ());
    mock_mixer
        .expect_remove_source()
        .with(eq(track_id_2))
        .returning(|_| ());

    mock_state_service
        .expect_modify_session()
        .with(eq(guild_id), always())
        .times(0)
        .returning(|_, _f| Ok(()));

    mock_state_service
        .expect_save_session()
        .returning(|_| Ok(()));

    let session_control = SessionControl {
        guild_id,
        mixer: mock_mixer,
        decoder: mock_decoder,
        end_tx,
        state_service: mock_state_service,
        taphub_service: mock_taphub_service,
    };

    let result = session_control.stop_many(AudioStopFilter::All).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_next_music() {
    let mut mock_state_service = MockStateService::new();
    let mock_taphub_service = MockTapHubService::new();
    let mut mock_mixer = MockMixer::new();
    let mock_decoder = MockDecoder::new();
    let (end_tx, _end_rx) = channel::unbounded();
    let guild_id = GuildId::from(1);
    let current_track_id = TrackId::from(201);
    let next_track_id = TrackId::from(202);

    mock_state_service
        .expect_get_session()
        .with(eq(guild_id))
        .returning(move |_| {
            let mut queues = HashMap::new();
            queues.insert(
                QueueName::from("music".to_string()),
                vec![
                    Track {
                        track_id: current_track_id,
                        request: AudioRequest {
                            tap_name: TapName::from("t".to_string()),
                            request: AudioRequestString::from("r".to_string()),
                        },
                        volume: Volume::from(1.0),
                        queue_name: QueueName::from("music".to_string()),
                    },
                    Track {
                        track_id: next_track_id,
                        request: AudioRequest {
                            tap_name: TapName::from("t".to_string()),
                            request: AudioRequestString::from("r".to_string()),
                        },
                        volume: Volume::from(1.0),
                        queue_name: QueueName::from("music".to_string()),
                    },
                ],
            );
            Ok(Some(SessionState {
                guild_id: GuildId::from(1),
                channel_id: crate::types::ChannelId::from(1),
                queues,
            }))
        });

    mock_mixer
        .expect_remove_source()
        .with(eq(current_track_id))
        .returning(|_| ());

    mock_state_service
        .expect_modify_session()
        .with(eq(guild_id), always())
        .returning(|_, _f| Ok(()));

    mock_mixer.expect_has_source().returning(|_| true); // Assume playing to avoid triggering play_now logic

    let session_control = SessionControl {
        guild_id,
        mixer: mock_mixer,
        decoder: mock_decoder,
        end_tx,
        state_service: mock_state_service,
        taphub_service: mock_taphub_service,
    };

    let result = session_control.next_music().await;
    assert!(result.is_ok());
}
