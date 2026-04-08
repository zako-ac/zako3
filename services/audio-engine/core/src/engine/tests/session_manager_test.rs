use std::sync::Arc;

use crate::{
    engine::session_manager::SessionManager,
    service::{discord::MockDiscordService, state::MockStateService, taphub::MockTapHubService},
    types::{ChannelId, GuildId, SessionState},
};
use mockall::predicate::*;

#[tokio::test]
async fn test_session_manager_join() {
    let guild_id = GuildId::from(1);
    let channel_id = ChannelId::from(100);

    let mut mock_discord = MockDiscordService::new();
    let mut mock_state = MockStateService::new();
    let mock_taphub = MockTapHubService::new();

    // 1. Join voice channel
    mock_discord
        .expect_join_voice_channel()
        .with(eq(guild_id), eq(channel_id))
        .times(1)
        .returning(|_, _| Ok(()));

    // 2. Save session
    mock_state
        .expect_save_session()
        .withf(move |s| s.guild_id == guild_id && s.channel_id == channel_id)
        .times(1)
        .returning(|_| Ok(()));

    // 3. Initiate session -> play_audio (empty stream)
    mock_discord
        .expect_play_audio()
        .with(eq(guild_id), always())
        .times(1)
        .returning(|_, _| Ok(()));

    let manager = SessionManager::new(
        Arc::new(mock_discord),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
    );

    let res = manager.join(guild_id, channel_id).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_session_manager_leave() {
    let guild_id = GuildId::from(2);

    let mut mock_discord = MockDiscordService::new();
    let mut mock_state = MockStateService::new();
    let mock_taphub = MockTapHubService::new();

    // 1. Leave voice channel
    mock_discord
        .expect_leave_voice_channel()
        .with(eq(guild_id))
        .times(1)
        .returning(|_| Ok(()));

    let channel_id = ChannelId::from(200);

    // 2. Delete session
    mock_state
        .expect_delete_session()
        .with(eq(guild_id), eq(channel_id))
        .times(1)
        .returning(|_, _| Ok(()));

    let manager = SessionManager::new(
        Arc::new(mock_discord),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
    );

    let res = manager.leave(guild_id, channel_id).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_session_manager_get_sessions_in_guild() {
    let guild_id = GuildId::from(3);
    let session = SessionState {
        guild_id,
        channel_id: ChannelId::from(300),
        queues: Default::default(),
    };

    let mock_discord = MockDiscordService::new();
    let mut mock_state = MockStateService::new();
    let mock_taphub = MockTapHubService::new();

    mock_state
        .expect_list_sessions_in_guild()
        .with(eq(guild_id))
        .times(1)
        .returning(move |_| Ok(vec![session.clone()]));

    let manager = SessionManager::new(
        Arc::new(mock_discord),
        Arc::new(mock_state),
        Arc::new(mock_taphub),
    );

    let res = manager.get_sessions_in_guild(guild_id).await;
    assert!(res.is_ok());
    assert_eq!(res.unwrap().len(), 1);
}
