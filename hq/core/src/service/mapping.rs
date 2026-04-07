use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use hex;
use hq_types::{
    ChannelId, GuildId,
    hq::mapper::{
        EvaluateResultDto, MapperInputData, MapperStepResultDto, PipelineOrderDto, WasmMapperDto,
    },
    hq::settings::{EmojiMappingRule, TextMappingRule},
    hq::user::DiscordUserId,
};
use sha2::{Digest, Sha256};
use zako3_tts_matching::{
    ChannelInfo, MapperInputData as TtsMapperInputData, MapperStepResult, TtsMatchingService,
    UserInfo, WasmMapper,
    service::{DiscordInfoProvider, ProcessContext},
};

use crate::{CoreError, CoreResult, service::DiscordNameResolverSlot};

struct HqDiscordInfoProvider {
    resolver: DiscordNameResolverSlot,
    guild_id: u64,
}

#[async_trait]
impl DiscordInfoProvider for HqDiscordInfoProvider {
    async fn get_channel_info(&self, channel_id: ChannelId) -> Option<ChannelInfo> {
        let resolver = self.resolver.get()?;
        let name = resolver.channel_name(self.guild_id, channel_id.into())?;
        Some(ChannelInfo { name })
    }

    async fn get_user_info(&self, _user_id: &DiscordUserId) -> Option<UserInfo> {
        None
    }
}

fn to_tts_input(data: &MapperInputData) -> TtsMapperInputData {
    match data {
        MapperInputData::MappingList => TtsMapperInputData::MappingList,
        MapperInputData::DiscordInfo => TtsMapperInputData::DiscordInfo,
        MapperInputData::CallerInfo => TtsMapperInputData::CallerInfo,
        MapperInputData::MapperList => TtsMapperInputData::MapperList,
    }
}

fn from_tts_input(data: &TtsMapperInputData) -> MapperInputData {
    match data {
        TtsMapperInputData::MappingList => MapperInputData::MappingList,
        TtsMapperInputData::DiscordInfo => MapperInputData::DiscordInfo,
        TtsMapperInputData::CallerInfo => MapperInputData::CallerInfo,
        TtsMapperInputData::MapperList => MapperInputData::MapperList,
    }
}

fn step_to_dto(s: MapperStepResult) -> MapperStepResultDto {
    MapperStepResultDto {
        mapper_id: s.mapper_id,
        mapper_name: s.mapper_name,
        input_text: s.input_text,
        output_text: s.output_text,
        success: s.success,
        error: s.error,
    }
}

fn mapper_to_dto(m: WasmMapper) -> WasmMapperDto {
    WasmMapperDto {
        id: m.id,
        name: m.name,
        wasm_filename: m.wasm_filename,
        sha256_hash: m.sha256_hash,
        input_data: m.input_data.iter().map(from_tts_input).collect(),
    }
}

fn tts_error(e: zako3_tts_matching::Error) -> CoreError {
    match e {
        zako3_tts_matching::Error::NotFound(msg) => CoreError::NotFound(msg),
        other => CoreError::Internal(other.to_string()),
    }
}

#[derive(Clone)]
pub struct MappingService {
    inner: Arc<TtsMatchingService>,
    wasm_dir: PathBuf,
    resolver: DiscordNameResolverSlot,
}

impl MappingService {
    pub async fn new(
        wasm_dir: PathBuf,
        db_path: PathBuf,
        resolver: DiscordNameResolverSlot,
    ) -> CoreResult<Self> {
        tokio::fs::create_dir_all(&wasm_dir)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))?;

        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| CoreError::Internal(e.to_string()))?;
        }

        let inner = TtsMatchingService::new(wasm_dir.clone(), db_path)
            .await
            .map_err(tts_error)?;

        Ok(Self {
            inner: Arc::new(inner),
            wasm_dir,
            resolver,
        })
    }

    pub async fn list_mappers(&self) -> CoreResult<Vec<WasmMapperDto>> {
        self.inner
            .mapper_repo()
            .list_all()
            .await
            .map(|ms| ms.into_iter().map(mapper_to_dto).collect())
            .map_err(tts_error)
    }

    pub async fn get_mapper(&self, id: &str) -> CoreResult<WasmMapperDto> {
        self.inner
            .mapper_repo()
            .find_by_id(id)
            .await
            .map_err(tts_error)?
            .map(mapper_to_dto)
            .ok_or_else(|| CoreError::NotFound(format!("Mapper '{}' not found", id)))
    }

    pub async fn create_mapper(
        &self,
        id: String,
        name: String,
        input_data: Vec<MapperInputData>,
        wasm_bytes: Vec<u8>,
    ) -> CoreResult<WasmMapperDto> {
        let filename = format!("{}.wasm", id);
        let path = self.wasm_dir.join(&filename);
        let hash = hex::encode(Sha256::digest(&wasm_bytes));

        tokio::fs::write(&path, &wasm_bytes)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))?;

        let mapper = WasmMapper {
            id: id.clone(),
            name,
            wasm_filename: filename,
            sha256_hash: hash,
            input_data: input_data.iter().map(to_tts_input).collect(),
        };

        let created = self
            .inner
            .mapper_repo()
            .create(mapper)
            .await
            .map_err(tts_error)?;

        // Validate the WASM runs correctly with a dummy input
        let validation = self
            .evaluate_mapper(id.clone(), "validation_test".to_string())
            .await;
        match validation {
            Ok(result) if result.steps.first().map(|s| s.success).unwrap_or(false) => {
                // Validation passed — return the DTO
                Ok(mapper_to_dto(created))
            }
            Ok(result) => {
                // Mapper ran but reported an error
                let err_msg = result
                    .steps
                    .first()
                    .and_then(|s| s.error.as_deref())
                    .unwrap_or("mapper produced no output")
                    .to_string();
                // Roll back
                let _ = self.inner.mapper_repo().delete(&id).await;
                let _ = tokio::fs::remove_file(&path).await;
                Err(CoreError::InvalidInput(format!(
                    "Mapper validation failed: {}",
                    err_msg
                )))
            }
            Err(e) => {
                // evaluate_mapper itself failed (e.g. WASM is not a valid module)
                let _ = self.inner.mapper_repo().delete(&id).await;
                let _ = tokio::fs::remove_file(&path).await;
                Err(CoreError::InvalidInput(format!(
                    "Mapper validation failed: {}",
                    e
                )))
            }
        }
    }

    pub async fn update_mapper(
        &self,
        id: String,
        name: String,
        input_data: Vec<MapperInputData>,
        wasm_bytes: Option<Vec<u8>>,
    ) -> CoreResult<WasmMapperDto> {
        let existing = self
            .inner
            .mapper_repo()
            .find_by_id(&id)
            .await
            .map_err(tts_error)?
            .ok_or_else(|| CoreError::NotFound(format!("Mapper '{}' not found", id)))?;

        let (wasm_filename, sha256_hash) = if let Some(bytes) = wasm_bytes {
            let filename = format!("{}.wasm", id);
            let path = self.wasm_dir.join(&filename);
            let hash = hex::encode(Sha256::digest(&bytes));
            tokio::fs::write(&path, &bytes)
                .await
                .map_err(|e| CoreError::Internal(e.to_string()))?;
            (filename, hash)
        } else {
            (existing.wasm_filename, existing.sha256_hash)
        };

        let mapper = WasmMapper {
            id,
            name,
            wasm_filename,
            sha256_hash,
            input_data: input_data.iter().map(to_tts_input).collect(),
        };

        self.inner
            .mapper_repo()
            .update(mapper)
            .await
            .map(mapper_to_dto)
            .map_err(tts_error)
    }

    pub async fn delete_mapper(&self, id: &str) -> CoreResult<()> {
        if let Ok(Some(mapper)) = self.inner.mapper_repo().find_by_id(id).await {
            let path = self.wasm_dir.join(&mapper.wasm_filename);
            let _ = tokio::fs::remove_file(&path).await;
        }

        self.inner.mapper_repo().delete(id).await.map_err(tts_error)
    }

    pub async fn get_pipeline(&self) -> CoreResult<PipelineOrderDto> {
        self.inner
            .pipeline_repo()
            .get_ordered()
            .await
            .map(|ids| PipelineOrderDto { mapper_ids: ids })
            .map_err(tts_error)
    }

    pub async fn set_pipeline(&self, ids: Vec<String>) -> CoreResult<()> {
        self.inner
            .pipeline_repo()
            .set_ordered(&ids)
            .await
            .map_err(tts_error)
    }

    /// Evaluate a specific list of mappers step-by-step on the given text.
    ///
    /// Uses `mapper_ids` directly (ignores the stored pipeline order), so callers
    /// can pass an unsaved local order for preview purposes.
    pub async fn evaluate_pipeline(
        &self,
        text: String,
        mapper_ids: Vec<String>,
    ) -> CoreResult<EvaluateResultDto> {
        let ctx = self.dummy_context(text);
        let (final_text, steps) = self
            .inner
            .process_with_ids(ctx, &mapper_ids)
            .await
            .map_err(tts_error)?;

        Ok(EvaluateResultDto {
            final_text,
            steps: steps.into_iter().map(step_to_dto).collect(),
        })
    }

    /// Test a single registered mapper in isolation.
    pub async fn evaluate_mapper(
        &self,
        mapper_id: String,
        text: String,
    ) -> CoreResult<EvaluateResultDto> {
        self.evaluate_pipeline(text, vec![mapper_id]).await
    }

    fn dummy_context(&self, text: String) -> ProcessContext {
        let discord_info = Arc::new(HqDiscordInfoProvider {
            resolver: self.resolver.clone(),
            guild_id: 0,
        });
        ProcessContext {
            text,
            guild_id: GuildId::from(0u64),
            channel_id: ChannelId::from(0u64),
            caller: DiscordUserId("0".to_string()),
            text_mappings: vec![],
            emoji_mappings: vec![],
            discord_info,
        }
    }

    pub async fn map_text(
        &self,
        text: String,
        guild_id: GuildId,
        channel_id: ChannelId,
        caller: DiscordUserId,
        text_mappings: Vec<TextMappingRule>,
        emoji_mappings: Vec<EmojiMappingRule>,
    ) -> CoreResult<String> {
        let discord_info = Arc::new(HqDiscordInfoProvider {
            resolver: self.resolver.clone(),
            guild_id: guild_id.into(),
        });

        let ctx = ProcessContext {
            text,
            guild_id,
            channel_id,
            caller,
            text_mappings,
            emoji_mappings,
            discord_info,
        };

        self.inner.process(ctx).await.map_err(tts_error)
    }
}
