use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use zako3_tts_matching::{
    ChannelInfo, DiscordInfoProvider, MapperRepository, PipelineRepository, ProcessContext,
    TtsMatchingService, UserInfo, WasmMapper,
};
use zako3_types::{
    ChannelId, GuildId,
    hq::user::DiscordUserId,
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

#[derive(Default)]
struct InMemoryRepo {
    mappers: Mutex<HashMap<String, WasmMapper>>,
    pipeline: Mutex<Vec<String>>,
}

#[async_trait]
impl MapperRepository for InMemoryRepo {
    async fn create(&self, mapper: WasmMapper) -> zako3_tts_matching::Result<WasmMapper> {
        self.mappers.lock().unwrap().insert(mapper.id.clone(), mapper.clone());
        Ok(mapper)
    }

    async fn find_by_id(&self, id: &str) -> zako3_tts_matching::Result<Option<WasmMapper>> {
        Ok(self.mappers.lock().unwrap().get(id).cloned())
    }

    async fn update(&self, mapper: WasmMapper) -> zako3_tts_matching::Result<WasmMapper> {
        self.mappers.lock().unwrap().insert(mapper.id.clone(), mapper.clone());
        Ok(mapper)
    }

    async fn delete(&self, id: &str) -> zako3_tts_matching::Result<()> {
        self.mappers.lock().unwrap().remove(id);
        Ok(())
    }

    async fn list_all(&self) -> zako3_tts_matching::Result<Vec<WasmMapper>> {
        Ok(self.mappers.lock().unwrap().values().cloned().collect())
    }
}

#[async_trait]
impl PipelineRepository for InMemoryRepo {
    async fn get_ordered(&self) -> zako3_tts_matching::Result<Vec<String>> {
        Ok(self.pipeline.lock().unwrap().clone())
    }

    async fn set_ordered(&self, mapper_ids: &[String]) -> zako3_tts_matching::Result<()> {
        *self.pipeline.lock().unwrap() = mapper_ids.to_vec();
        Ok(())
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

fn make_service() -> (TtsMatchingService, Arc<InMemoryRepo>) {
    let repo = Arc::new(InMemoryRepo::default());
    let service = TtsMatchingService::new(repo.clone(), repo.clone()).unwrap();
    (service, repo)
}

#[tokio::test]
async fn lowercase_mapper_transforms_text() {
    let (service, _repo) = make_service();

    let mapper = service
        .mapper_repo()
        .create(WasmMapper {
            id: "lowercase".to_string(),
            name: "Lowercase".to_string(),
            wasm_bytes: LOWERCASE_WASM.to_vec(),
            sha256_hash: wasm_hash(LOWERCASE_WASM),
            input_data: vec![],
        })
        .await
        .unwrap();

    service
        .pipeline_repo()
        .set_ordered(std::slice::from_ref(&mapper.id))
        .await
        .unwrap();

    let result = service.process(ctx("Hello World")).await.unwrap();
    assert_eq!(result, "hello world");
}

#[tokio::test]
async fn empty_pipeline_returns_text_unchanged() {
    let (service, _repo) = make_service();

    let result = service.process(ctx("Hello World")).await.unwrap();
    assert_eq!(result, "Hello World");
}

#[tokio::test]
async fn mapper_with_wrong_hash_skips() {
    let (service, _repo) = make_service();

    service
        .mapper_repo()
        .create(WasmMapper {
            id: "lowercase".to_string(),
            name: "Lowercase".to_string(),
            wasm_bytes: LOWERCASE_WASM.to_vec(),
            sha256_hash: "0".repeat(64),
            input_data: vec![],
        })
        .await
        .unwrap();

    service
        .pipeline_repo()
        .set_ordered(&["lowercase".to_string()])
        .await
        .unwrap();

    let result = service.process(ctx("Hello World")).await.unwrap();
    assert_eq!(result, "Hello World");
}
