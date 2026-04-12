// File: services/traffic-light/core/src/service/tests.rs

use std::sync::{Arc, RwLock};
use mockall::mock;
use tokio::sync::Mutex;
use zako3_tl_core::{
    AeDispatcher, AeId, SessionRoute, Worker, WorkerId, WorkerPermissions, ZakoState, etc.
}; 
// Assume necessary imports like SessionInfo, AudioEngineCommand, TlError, etc. are available via `use` at the top of the file.
// For this mock, we'll mock the structure based on the plan.

// --- Mocks and Helper State Builders (as described in plan) ---

// Mock the dispatcher trait for unit testing
mock! {
    pub struct MockAeDispatcher(AeDispatcher);
}

#[async_trait::async_trait]
impl AeDispatcher for MockAeDispatcher {
    async fn send(
        &self,
        route: SessionRoute,
        request: AudioEngineCommandRequest,
    ) -> Result<AudioEngineCommandResponse, TlError> {
        self.0.send(route, request).await
    }
}

// Re-using helper functions definitions from the plan/context if they weren't in the file.
// In a real project, these would either be accessible or defined here.
fn route() -> SessionRoute { SessionRoute { worker_id: WorkerId(0), ae_id: AeId(1) } }
fn session(g: u64, c: u64) -> SessionInfo { SessionInfo { guild_id: GuildId::from(g), channel_id: ChannelId::from(c) } }


// Helper function state_with_connected_ae (AE connected, no sessions in ZakoState)
fn state_with_connected_ae() -> Arc<RwLock<ZakoState>> {
    // Placeholder implementation matching plan assumption
    let worker = Worker {
        worker_id: WorkerId(0),
        bot_client_id: zako3_types::hq::DiscordUserId(String::new()),
        discord_token: tac(String::new()), // Placeholder for DiscordToken
        connected_ae_ids: vec![1],
        permissions: WorkerPermissions::new(),
        ae_cursor: 0,
    };
    let mut workers: rustc_hash::FxHashMap<WorkerId, Worker> = Default::default();
    workers.insert(WorkerId(0), worker);
    Arc::new(RwLock::new(ZakoState {
        workers,
        sessions: Default::default(),
        worker_cursor: 0,
    }))
}

// Helper function state_with_session (AE connected + a specific session registered)
fn state_with_session(session: SessionInfo) -> Arc<RwLock<ZakoState>> {
    // Placeholder implementation matching plan assumption
    let state = state_with_connected_ae();
    let mut write_state = state.write().unwrap();
    write_state.sessions.insert(route(), session);
    state
}


#[tokio::test]
async fn reconcile_no_connected_aes_does_nothing() {
    // Setup: State with connected AE, but no sessions cached
    let state = state_with_connected_ae();
    let mock_dispatcher = MockAeDispatcher(mock::Mock::new());
    let mock_dispatcher_arc: Arc<dyn AeDispatcher<Get = Send + Sync>> = Arc::new(mock_dispatcher);

    // Expectations: No calls to send()
    mock_dispatcher.expect_send().times(0);

    // Action
    let service = TlService::new(state.clone(), mock_dispatcher_arc);
    service.reconcile().await;

    // Verification is implicit through mock expectations
}

#[tokio::test]
async fn reconcile_tl_restart_leaves_dangling() {
    // Setup: TL Restart -> State has AE connected, NO sessions. AE reports bot is still in a VC.
    let state = state_with_connected_ae();
    let mock_dispatcher = MockAeDispatcher(mock::Mock::new());
    let mock_dispatcher_arc: Arc<dyn AeDispatcher<Get = Send + Sync>> = Arc::new(mock_dispatcher);
    let service = TlService::new(state.clone(), mock_dispatcher_arc);

    let discord_session = session(1, 100);

    // Expectations:
    // 1. FetchDiscordVoiceState is called once.
    mock_dispatcher.expect_send()
        .times(1)
        .returning(move |_, req| {
            // Assert command is FetchDiscordVoiceState
            assert!(matches!(req.command, AudioEngineCommand::FetchDiscordVoiceState));
            Ok(AudioEngineCommandResponse::DiscordVoiceState(vec![discord_session]))
        });
    // 2. Leave is sent for the dangling session.
    mock_dispatcher.expect_send()
        .times(1)
        .withf(|_, req| req.session == Some(discord_session)
            && matches!(req.command, AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Leave)))
        .returning(|_, _| Ok(AudioEngineCommandResponse::Ok));

    // Action
    service.reconcile().await;
}

#[tokio::test]
async fn reconcile_ae_restart_clears_and_leaves_dangling() {
    // Setup: AE Restart -> State has connected AE, but a stale session was already cleared by sync_sessions.
    // This scenario should mirror TL restart logic paths but use explicit state setup.
    let state = state_with_connected_ae();
    let mock_dispatcher = MockAeDispatcher(mock::Mock::new());
    let mock_dispatcher_arc: Arc<dyn AeDispatcher<Get = Send + Sync>> = Arc::new(mock_dispatcher);
    let service = TlService::new(state.clone(), mock_dispatcher_arc);

    let discord_session = session(1, 100);

    // Expectations: Same as TL Restart scenario
    mock_dispatcher.expect_send()
        .times(1)
        .returning(move |_, req| {
            assert!(matches!(req.command, AudioEngineCommand::FetchDiscordVoiceState));
            Ok(AudioEngineCommandResponse::DiscordVoiceState(vec![discord_session]))
        });
    mock_dispatcher.expect_send()
        .times(1)
        .withf(|_, req| req.session == Some(discord_session)
            && matches!(req.command, AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Leave)))
        .returning(|_, _| Ok(AudioEngineCommandResponse::Ok));

    // Action
    service.reconcile().await;
}


#[tokio::test]
async fn reconcile_matching_sessions_no_leave() {
    // Setup: State has a session AND Discord reports that same session. Bot is properly joined.
    let s = session(1, 100);
    let state = state_with_session(s);
    let mock_dispatcher = MockAeDispatcher(mock::Mock::new());
    let mock_dispatcher_arc: Arc<dyn AeDispatcher<Get = Send + Sync>> = Arc::new(mock_dispatcher);
    let service = TlService::new(state.clone(), mock_dispatcher_arc);

    // Expectations: Only FetchDiscordVoiceState is called. No Leave.
    mock_dispatcher.expect_send()
        .times(1)
        .returning(move |_, _| Ok(AudioEngineCommandResponse::DiscordVoiceState(vec![s])));
    
    // Action
    service.reconcile().await;
}

#[tokio::test]
async fn reconcile_partial_dangling() {
    // Setup: AE reports 2 sessions. Only 1 is cached in TL. The other is dangling — Leave for the dangling one only.
    let cached = session(1, 100);
    let dangling = session(2, 200);
    let state = state_with_session(cached);
    let mock_dispatcher = MockAeDispatcher(mock::Mock::new());
    let mock_dispatcher_arc: Arc<dyn AeDispatcher<Get = Send + Sync>> = Arc::new(mock_dispatcher);
    let service = TlService::new(state.clone(), mock_dispatcher_arc);
    
    // Setup Sequence for mocking multiple calls
    use mockall::predicate::* ;
    let mut seq = mock::Sequence::new();

    // Expectations:
    // 1. FetchDiscordVoiceState → returns both
    mock_dispatcher.expect_send()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(AudioEngineCommandResponse::DiscordVoiceState(vec![cached, dangling])));
    // 2. Leave → only for dangling
    mock_dispatcher.expect_send()
        .times(1)
        .in_sequence(&mut seq)
        .withf(|_, req| req.session == Some(dangling)
            && matches!(req.command, AudioEngineCommand::SessionCommand(AudioEngineSessionCommand::Leave)))
        .returning(|_, _| Ok(AudioEngineCommandResponse::Ok));

    // Action
    service.reconcile().await;
}

#[tokio::test]
async fn reconcile_fetch_dispatch_error_continues() {
    // Setup: FetchDiscordVoiceState dispatch fails (AE unreachable). Reconcile logs warning and does not panic.
    let state = state_with_connected_ae();
    let mock_dispatcher = MockAeDispatcher(mock::Mock::new());
    let mock_dispatcher_arc: Arc<dyn AeDispatcher<Get = Send + Sync>> = Arc::new(mock_dispatcher);
    let service = TlService::new(state.clone(), mock_dispatcher_arc);

    // Expectations: Only one call expected, which fails. No Leave sent.
    mock_dispatcher.expect_send()
        .times(1)
        .returning(|_, _| Err(TlError::Transport("AE dead".into())));

    // Action
    service.reconcile().await;
}

#[tokio::test]
async fn reconcile_unexpected_response_continues() {
    // Setup: AE returns Ok instead of DiscordVoiceState. Reconcile logs warning and does not panic, no Leave sent.
    let state = state_with_connected_ae();
    let mock_dispatcher = MockAeDispatcher(mock::Mock::new());
    let mock_dispatcher_arc: Arc<dyn AeDispatcher<Get = Send + Sync>> = Arc::new(mock_dispatcher);
    let service = TlService::new(state.clone(), mock_dispatcher_arc);

    // Expectations: Only one call expected, which returns Ok unexpectedly.
    mock_dispatcher.expect_send()
        .times(1)
        .returning(|_, _| Ok(AudioEngineCommandResponse::Ok));

    // Action
    service.reconcile().await;
}
// Dummy definitions needed for compilation context based on the plan
mod zako3_types {
    pub use zako3_utils::{ChannelId, GuildId};
}
// Mocking missing types for successful file write:
// Needs actual definition in the real project.
pub type TlError = String; 
pub type discord_token = String;
pub struct DiscordToken(pub discord_token);
impl From<String> for DiscordToken { fn from(s: String) -> Self { DiscordToken(s) } }

// Mocking TlService and dependencies needed by the test module struct
pub struct TlService {
    // ... fields
}
impl TlService {
    pub fn new(_state: Arc<RwLock<ZakoState>>, _dispatcher: Arc<dyn AeDispatcher<Get = Send + Sync>>) -> TlService { TlService {} }
    pub async fn reconcile(&self) {}
}
// Placeholder: Actual implementation relies on services being fully available.