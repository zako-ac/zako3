use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tl_protocol::{AudioEngineCommandRequest, AudioEngineCommandResponse};
use zako3_ae_transport::{DiscordToken, TlClient, TlClientHandler, TlServer};

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
        session: SessionInfo {
            guild_id: GuildId::from(1u64),
            channel_id: ChannelId::from(2u64),
        },
        command: AudioEngineCommand::Join,
        headers: HashMap::new(),
        idempotency_key: None,
    }
}

#[tokio::test]
async fn handshake_assigns_token() {
    let mut server = TlServer::bind("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        server
            .accept(DiscordToken("test-token".into()), HashMap::new())
            .await
            .unwrap()
    });

    let (token, _headers, _connected) = TlClient::connect(addr, HashMap::new()).await.unwrap();

    let _server_client = server_task.await.unwrap();
    assert_eq!(token.0, "test-token");
}

#[tokio::test]
async fn request_response_roundtrip() {
    let mut server = TlServer::bind("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        server
            .accept(DiscordToken("tok".into()), HashMap::new())
            .await
            .unwrap()
    });

    let (_token, _headers, connected) = TlClient::connect(addr, HashMap::new()).await.unwrap();
    tokio::spawn(async move { connected.serve(Arc::new(MockHandler)).await });

    let mut server_client = server_task.await.unwrap();
    let resp = server_client.request(make_request()).await.unwrap();
    assert!(matches!(resp, AudioEngineCommandResponse::Ok));
}

#[tokio::test]
async fn multiple_sequential_requests() {
    let mut server = TlServer::bind("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        server
            .accept(DiscordToken("tok".into()), HashMap::new())
            .await
            .unwrap()
    });

    let (_token, _headers, connected) = TlClient::connect(addr, HashMap::new()).await.unwrap();
    tokio::spawn(async move { connected.serve(Arc::new(MockHandler)).await });

    let mut server_client = server_task.await.unwrap();
    for _ in 0..3 {
        let resp = server_client.request(make_request()).await.unwrap();
        assert!(matches!(resp, AudioEngineCommandResponse::Ok));
    }
}
