use std::sync::Arc;

use async_trait::async_trait;
use jsonrpsee::core::RpcResult;
use jsonrpsee::server::Server;
use tl_protocol::{AudioEngineCommandRequest, AudioEngineCommandResponse, AudioEngineRpcServer, SessionInfo};
use tokio::sync::{Mutex, RwLock};
use zako3_tl_core::{AeDispatcher, AeId, DiscordToken, SessionRoute, Worker, WorkerId, WorkerPermissions, ZakoState};
use zako3_tl_infra::AeRegistry;
use zako3_types::{ChannelId, GuildId};

fn make_state_with_token(token: &str) -> Arc<RwLock<ZakoState>> {
    let worker = Worker {
        worker_id: WorkerId(0),
        bot_client_id: zako3_types::hq::DiscordUserId(String::new()),
        discord_token: DiscordToken(token.to_string()),
        connected_ae_ids: vec![],
        permissions: WorkerPermissions::new(),
    };
    let mut workers: rustc_hash::FxHashMap<WorkerId, Worker> = Default::default();
    workers.insert(WorkerId(0), worker);
    Arc::new(RwLock::new(ZakoState {
        workers,
        sessions: Default::default(),
    }))
}

fn make_state_with_two_tokens(token_a: &str, token_b: &str) -> Arc<RwLock<ZakoState>> {
    let mut workers: rustc_hash::FxHashMap<WorkerId, Worker> = Default::default();
    workers.insert(WorkerId(0), Worker {
        worker_id: WorkerId(0),
        bot_client_id: zako3_types::hq::DiscordUserId(String::new()),
        discord_token: DiscordToken(token_a.to_string()),
        connected_ae_ids: vec![],
        permissions: WorkerPermissions::new(),
    });
    workers.insert(WorkerId(1), Worker {
        worker_id: WorkerId(1),
        bot_client_id: zako3_types::hq::DiscordUserId(String::new()),
        discord_token: DiscordToken(token_b.to_string()),
        connected_ae_ids: vec![],
        permissions: WorkerPermissions::new(),
    });
    Arc::new(RwLock::new(ZakoState {
        workers,
        sessions: Default::default(),
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

/// When an AE restarts it calls register() fresh (token lost). Without the fix, round-robin
/// would assign worker_id=1 on the second call, leaving a stale client and session on worker_id=0
/// that sync_sessions would never clean because the same AE still responds there.
/// With the fix, the second register() for the same address reuses worker_id=0, and the old
/// client+session is evicted before the new one is inserted.
#[tokio::test]
async fn re_register_same_addr_keeps_worker_id_and_clears_sessions() {
    let token_a = "token-worker-0";
    let token_b = "token-worker-1";
    let state = make_state_with_two_tokens(token_a, token_b);
    let tokens = vec![
        DiscordToken(token_a.to_string()),
        DiscordToken(token_b.to_string()),
    ];

    let registry = Arc::new(AeRegistry::new(state.clone(), tokens).await.unwrap());

    let server = Server::builder().build("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    let listen_addr = format!("http://{}", addr);
    tokio::spawn(async move { server.start(MockAeHandler.into_rpc()).stopped().await });

    // First registration: should pick worker_id=0 (cursor=0)
    let assigned_token = registry.register(listen_addr.clone()).await.unwrap();
    assert_eq!(assigned_token, token_a, "first registration should get token_a (worker_id=0)");

    // Inject a cached session under the assigned route to simulate an active session
    {
        let mut s = state.write().await;
        s.sessions.insert(
            SessionRoute { worker_id: WorkerId(0), ae_id: AeId(1) },
            SessionInfo { guild_id: GuildId::from(42u64), channel_id: ChannelId::from(99u64) },
        );
    }

    // Second registration from the same address (simulating AE restart, token forgotten)
    let assigned_token2 = registry.register(listen_addr.clone()).await.unwrap();
    assert_eq!(assigned_token2, token_a, "re-registration must reuse same token/worker_id");

    let s = state.read().await;

    // The stale session on worker_id=0 must have been evicted during re-registration
    let sessions_on_w0: Vec<_> = s.sessions.iter()
        .filter(|(r, _)| r.worker_id == WorkerId(0))
        .collect();
    assert!(sessions_on_w0.is_empty(), "stale sessions on worker_id=0 must be evicted on re-register");

    // worker_id=1 must still have no client (AE never registered there)
    let w1_connected = s.workers.get(&WorkerId(1)).map(|w| w.connected_ae_ids.len()).unwrap_or(0);
    assert_eq!(w1_connected, 0, "worker_id=1 should have no connected AEs");
}

/// Regression for the bot-token collision: the round-robin cursor can point at a token whose
/// worker already has a live AE (e.g. after AE restarts from changing addresses pushed the cursor
/// past the pool size). Plain `cursor % len` would hand that live token to the new AE, so two
/// engines log in as the same bot and Discord drops one — you see N-1 bots online. pick_free_token
/// must skip in-use tokens and assign a free one instead.
#[tokio::test]
async fn register_skips_token_already_live_on_another_ae() {
    let token_a = "token-worker-0";
    let token_b = "token-worker-1";
    let state = make_state_with_two_tokens(token_a, token_b);
    let tokens = vec![
        DiscordToken(token_a.to_string()),
        DiscordToken(token_b.to_string()),
    ];

    // Simulate worker_id=0 already serving a live AE; its token must not be handed out again.
    {
        let mut s = state.write().await;
        s.workers.get_mut(&WorkerId(0)).unwrap().connected_ae_ids = vec![1];
    }

    let registry = Arc::new(AeRegistry::new(state.clone(), tokens).await.unwrap());

    let server = Server::builder().build("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    let listen_addr = format!("http://{}", addr);
    tokio::spawn(async move { server.start(MockAeHandler.into_rpc()).stopped().await });

    // The cursor starts at 0 → points at token_a (worker_id=0), which is already in use.
    // The fix must skip it and assign the free token_b (worker_id=1).
    let assigned = registry.register(listen_addr).await.unwrap();
    assert_eq!(
        assigned, token_b,
        "registration must skip the live token_a and assign the free token_b"
    );
}

/// Regression for the worker_id=0,1,1,2 collision (worker 3 never assigned): AE pods advertise
/// their StatefulSet hostname (`zako3-audio-engine-<ordinal>`), and the ordinal must deterministically
/// pin the worker. Registration order, restarts, and pool state must not change the mapping, and two
/// distinct pods must never share a token.
#[tokio::test]
async fn statefulset_ordinal_pins_worker_and_never_collides() {
    let tokens: Vec<DiscordToken> = (0..4).map(|i| DiscordToken(format!("token-{i}"))).collect();

    let mut workers: rustc_hash::FxHashMap<WorkerId, Worker> = Default::default();
    for i in 0..4u16 {
        workers.insert(WorkerId(i), Worker {
            worker_id: WorkerId(i),
            bot_client_id: zako3_types::hq::DiscordUserId(String::new()),
            discord_token: DiscordToken(format!("token-{i}")),
            connected_ae_ids: vec![],
            permissions: WorkerPermissions::new(),
        });
    }
    let state = Arc::new(RwLock::new(ZakoState {
        workers,
        sessions: Default::default(),
    }));

    let registry = Arc::new(AeRegistry::new(state.clone(), tokens).await.unwrap());

    // register() only builds the HTTP client and updates state — no network call — so pod-style
    // hostnames need no live server. Register out of order, with a restart of pod-0 mixed in.
    let sequence = [(3usize, "token-3"), (0, "token-0"), (2, "token-2"), (1, "token-1"), (0, "token-0")];
    for (pod, expected) in sequence {
        let addr = format!(
            "http://zako3-audio-engine-{pod}.zako3-audio-engine.ns.svc.cluster.local:8090"
        );
        let assigned = registry.register(addr).await.unwrap();
        assert_eq!(
            assigned, expected,
            "pod-{pod} must always map to {expected} regardless of order/restart"
        );
    }

    // Every worker 0..=3 has exactly one connected AE — no collision, no empty slot.
    let s = state.read().await;
    for i in 0..4u16 {
        assert_eq!(
            s.workers.get(&WorkerId(i)).unwrap().connected_ae_ids,
            vec![1],
            "worker {i} must have exactly one AE"
        );
    }
}

/// Heartbeats refresh the HTTP client but must NOT evict active sessions. Without the
/// evict_sessions guard, heartbeats would wipe sessions every ~15s, causing reconcile to
/// see discord=1/cache=0 and send dangling kicks while the AE is perfectly healthy.
#[tokio::test]
async fn heartbeat_does_not_evict_sessions() {
    let token = "hb-token";
    let state = make_state_with_token(token);
    let tokens = vec![DiscordToken(token.to_string())];

    let registry = Arc::new(AeRegistry::new(state.clone(), tokens).await.unwrap());

    let server = Server::builder().build("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    let listen_addr = format!("http://{}", addr);
    tokio::spawn(async move { server.start(MockAeHandler.into_rpc()).stopped().await });

    let assigned_token = registry.register(listen_addr.clone()).await.unwrap();

    // Inject an active session (simulates a committed Join)
    {
        let mut s = state.write().await;
        s.sessions.insert(
            SessionRoute { worker_id: WorkerId(0), ae_id: AeId(1) },
            SessionInfo { guild_id: GuildId::from(7u64), channel_id: ChannelId::from(8u64) },
        );
    }

    // Simulate a heartbeat from the AE (same token, same address)
    registry.heartbeat(assigned_token, listen_addr).await.unwrap();

    let s = state.read().await;
    let sessions_on_w0: Vec<_> = s.sessions.iter()
        .filter(|(r, _)| r.worker_id == WorkerId(0))
        .collect();
    assert_eq!(sessions_on_w0.len(), 1, "heartbeat must not evict active sessions");
}
