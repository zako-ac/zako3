use std::sync::Arc;

use async_trait::async_trait;
use zako3_tts_matching::{
    ChannelInfo, DiscordInfoProvider, ProcessContext, TtsMatchingService, UserInfo, WasmMapper,
};
use zako3_types::{
    hq::user::DiscordUserId,
    {ChannelId, GuildId},
};

static LOWERCASE_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/lowercase.wasm"));

struct NoopDiscord;

#[async_trait]
impl DiscordInfoProvider for NoopDiscord {
    async fn get_channel_info(&self, _: ChannelId) -> Option<ChannelInfo> {
        None
    }

    async fn get_user_info(&self, _: &DiscordUserId) -> Option<UserInfo> {
        None
    }
}

fn ctx(text: &str) -> ProcessContext {
    ProcessContext {
        text: text.to_string(),
        guild_id: GuildId::from(1u64),
        channel_id: ChannelId::from(1u64),
        caller: DiscordUserId("user1".to_string()),
        text_mappings: vec![],
        emoji_mappings: vec![],
        discord_info: Arc::new(NoopDiscord),
    }
}

fn wasm_hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

#[tokio::test]
async fn lowercase_mapper_transforms_text() {
    let dir = tempfile::tempdir().unwrap();
    let wasm_path = dir.path().join("lowercase.wasm");
    std::fs::write(&wasm_path, LOWERCASE_WASM).unwrap();

    let service = TtsMatchingService::new(dir.path().to_path_buf(), dir.path().join("test.db"))
        .await
        .unwrap();

    let mapper = service
        .mapper_repo()
        .create(WasmMapper {
            id: "lowercase".to_string(),
            name: "Lowercase".to_string(),
            wasm_filename: "lowercase.wasm".to_string(),
            sha256_hash: wasm_hash(LOWERCASE_WASM),
            input_data: vec![],
        })
        .await
        .unwrap();

    service
        .pipeline_repo()
        .set_ordered(&[mapper.id.clone()])
        .await
        .unwrap();

    let result = service.process(ctx("Hello World")).await.unwrap();
    assert_eq!(result, "hello world");
}

#[tokio::test]
async fn empty_pipeline_returns_text_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let service = TtsMatchingService::new(dir.path().to_path_buf(), dir.path().join("test.db"))
        .await
        .unwrap();

    let result = service.process(ctx("Hello World")).await.unwrap();
    assert_eq!(result, "Hello World");
}

#[tokio::test]
async fn mapper_with_wrong_hash_skips() {
    let dir = tempfile::tempdir().unwrap();
    let wasm_path = dir.path().join("lowercase.wasm");
    std::fs::write(&wasm_path, LOWERCASE_WASM).unwrap();

    let service = TtsMatchingService::new(dir.path().to_path_buf(), dir.path().join("test.db"))
        .await
        .unwrap();

    service
        .mapper_repo()
        .create(WasmMapper {
            id: "lowercase".to_string(),
            name: "Lowercase".to_string(),
            wasm_filename: "lowercase.wasm".to_string(),
            sha256_hash: "0".repeat(64), // wrong hash
            input_data: vec![],
        })
        .await
        .unwrap();

    service
        .pipeline_repo()
        .set_ordered(&["lowercase".to_string()])
        .await
        .unwrap();

    // pipeline.rs:66 — hash mismatch logs a warning and silently skips the mapper
    let result = service.process(ctx("Hello World")).await.unwrap();
    assert_eq!(result, "Hello World");
}
