use std::sync::Arc;

use async_trait::async_trait;
use jsonrpsee::core::RpcResult;
use jsonrpsee::server::Server;
use tl_protocol::{AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineRpcServer};
use tokio::sync::{Mutex, RwLock};
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

struct MockAeHandler;

#[async_trait]
impl AudioEngineRpcServer for MockAeHandler {
    async fn execute(&self, _req: AudioEngineCommandRequest) -> RpcResult<AudioEngineCommandResponse> {
        Ok(AudioEngineCommandResponse::Ok)
    }
}

struct RecordingAeHandler(Arc<Mutex<Option<AudioEngineCommandRequest>>>);

#[async_trait]
impl AudioEngineRpcServer for RecordingAeHandler {
    async fn execute(&self, req: AudioEngineCommandRequest) -> RpcResult<AudioEngineCommandResponse> {
        *self.0.lock().await = Some(req);
        Ok(AudioEngineCommandResponse::Ok)
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
        headers: Default::default(),
        idempotency_key: None,
    }
}

#[tokio::test]
async fn ae_registers_in_state() {
    let token = "reg-token";
    let state = make_state_with_token(token);
    let tokens = vec![DiscordToken(token.to_string())];

    let registry = Arc::new(
        AeRegistry::new(state.clone(), tokens)
            .await
            .unwrap(),
    );

    // Start a mock HTTP server
    let server = Server::builder()
        .build("127.0.0.1:0")
        .await
        .unwrap();

    let addr = server.local_addr().unwrap();
    let listen_addr = format!("http://{}", addr);

    tokio::spawn({
        let server_handle = server.start(MockAeHandler.into_rpc());
        async move {
            server_handle.stopped().await;
        }
    });

    // Register the AE
    registry.register(listen_addr).await.unwrap();

    // Give time for registration to complete
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
        AeRegistry::new(state.clone(), tokens)
            .await
            .unwrap(),
    );

    // Recording handler
    let received: Arc<Mutex<Option<AudioEngineCommandRequest>>> = Arc::new(Mutex::new(None));
    let received_clone = received.clone();

    // Start a mock HTTP server
    let server = Server::builder()
        .build("127.0.0.1:0")
        .await
        .unwrap();

    let addr = server.local_addr().unwrap();
    let listen_addr = format!("http://{}", addr);

    tokio::spawn({
        let server_handle = server.start(RecordingAeHandler(received_clone).into_rpc());
        async move {
            server_handle.stopped().await;
        }
    });

    // Register the AE
    registry.register(listen_addr).await.unwrap();

    // Give time for registration to complete
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
