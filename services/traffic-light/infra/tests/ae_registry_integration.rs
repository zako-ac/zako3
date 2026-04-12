use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tl_protocol::{AudioEngineCommandRequest, AudioEngineCommandResponse};
use tokio::sync::{Mutex, RwLock};
use zako3_ae_transport::{TlClient, TlClientHandler};
use zako3_tl_core::{AeDispatcher, AeId, DiscordToken, SessionRoute, Worker, WorkerId, WorkerPermissions, ZakoState};
use zako3_tl_infra::AeRegistry;

fn make_state_with_token(token: &str) -> Arc<RwLock<ZakoState>> {
    let worker = Worker {
        worker_id: WorkerId(0),
        bot_client_id: zako3_types::hq::DiscordUserId(String::new()),
        discord_token: DiscordToken(token.to_string()),
        connected_ae_ids: vec![],
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

struct MockHandler;

#[async_trait]
impl TlClientHandler for MockHandler {
    async fn handle(
        &self,
        _req: AudioEngineCommandRequest,
        _headers: &HashMap<String, String>,
    ) -> AudioEngineCommandResponse {
        AudioEngineCommandResponse::Ok
    }
}

fn make_request() -> AudioEngineCommandRequest {
    use tl_protocol::{AudioEngineCommand, SessionInfo};
    use zako3_types::{ChannelId, GuildId};
    AudioEngineCommandRequest {
        session: Some(SessionInfo {
            guild_id: GuildId::from(1u64),
            channel_id: ChannelId::from(2u64),
        }),
        command: AudioEngineCommand::Join,
        headers: HashMap::new(),
        idempotency_key: None,
    }
}

#[tokio::test]
async fn ae_registers_in_state() {
    let token = "reg-token";
    let state = make_state_with_token(token);
    let tokens = vec![DiscordToken(token.to_string())];

    let registry = Arc::new(
        AeRegistry::new("127.0.0.1:0".parse().unwrap(), state.clone(), tokens)
            .await
            .unwrap(),
    );

    let addr = registry.local_addr().await.unwrap();

    // Spawn accept loop
    let reg_clone = registry.clone();
    tokio::spawn(async move { reg_clone.accept_loop().await });

    // Connect AE client and serve
    let (_token, _headers, connected) = TlClient::connect(addr, HashMap::new()).await.unwrap();
    tokio::spawn(async move { connected.serve(MockHandler).await });

    // Give accept loop time to process
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let state_guard = state.read().await;
    let worker = state_guard.workers.get(&WorkerId(0)).unwrap();
    assert!(
        !worker.connected_ae_ids.is_empty(),
        "AE should have registered its ID"
    );
    assert_eq!(worker.connected_ae_ids[0], 1);
}

#[tokio::test]
async fn dispatch_reaches_ae() {
    let token = "dispatch-token";
    let state = make_state_with_token(token);
    let tokens = vec![DiscordToken(token.to_string())];

    let registry = Arc::new(
        AeRegistry::new("127.0.0.1:0".parse().unwrap(), state.clone(), tokens)
            .await
            .unwrap(),
    );

    let addr = registry.local_addr().await.unwrap();

    let reg_clone = registry.clone();
    tokio::spawn(async move { reg_clone.accept_loop().await });

    // Recording handler
    let received: Arc<Mutex<Option<AudioEngineCommandRequest>>> = Arc::new(Mutex::new(None));
    let received_clone = received.clone();

    struct RecordingHandler(Arc<Mutex<Option<AudioEngineCommandRequest>>>);

    #[async_trait]
    impl TlClientHandler for RecordingHandler {
        async fn handle(
            &self,
            req: AudioEngineCommandRequest,
            _headers: &HashMap<String, String>,
        ) -> AudioEngineCommandResponse {
            *self.0.lock().await = Some(req);
            AudioEngineCommandResponse::Ok
        }
    }

    let (_token, _headers, connected) = TlClient::connect(addr, HashMap::new()).await.unwrap();
    tokio::spawn(async move { connected.serve(RecordingHandler(received_clone)).await });

    // Wait for registration
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let route = SessionRoute {
        worker_id: WorkerId(0),
        ae_id: AeId(1),
    };

    let resp = registry.send(route, make_request()).await.unwrap();
    assert!(matches!(resp, AudioEngineCommandResponse::Ok));

    // Verify the handler received the request
    let got = received.lock().await;
    assert!(got.is_some(), "handler should have received the request");
}
