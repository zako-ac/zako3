use std::sync::Arc;

use async_trait::async_trait;
use zako3_types::{
    ChannelId, GuildId,
    hq::settings::{EmojiMappingRule, TextMappingRule},
    hq::user::DiscordUserId,
};

use crate::{
    Result,
    model::{ChannelInfo, MapperStepResult, UserInfo},
    pipeline,
    repo::{MapperRepository, PipelineRepository},
    wasm::EngineState,
};

/// Provides Discord channel and user information to WASM mappers.
///
/// Mappers can optionally request Discord info by declaring `DiscordInfo` in their input data.
/// When a mapper queries Discord info, the host calls these methods to retrieve the data.
#[async_trait]
pub trait DiscordInfoProvider: Send + Sync {
    /// Get channel information by ID. Returns `None` if not found.
    async fn get_channel_info(&self, channel_id: ChannelId) -> Option<ChannelInfo>;

    /// Get user information by Discord user ID. Returns `None` if not found.
    async fn get_user_info(&self, user_id: &DiscordUserId) -> Option<UserInfo>;
}

/// Input context for text transformation processing.
///
/// Passed to `TtsMatchingService::process()`, this contains the TTS text to be transformed
/// along with all relevant metadata and configuration for the mapper pipeline.
pub struct ProcessContext {
    /// The TTS text to transform
    pub text: String,
    /// Guild ID where TTS will be played
    pub guild_id: GuildId,
    /// Channel ID where TTS will be played
    pub channel_id: ChannelId,
    /// Discord user ID of the TTS requester
    pub caller: DiscordUserId,
    /// Text mapping rules (if mapper requested `MappingList`)
    pub text_mappings: Vec<TextMappingRule>,
    /// Emoji mapping rules (if mapper requested `MappingList`)
    pub emoji_mappings: Vec<EmojiMappingRule>,
    /// Discord info provider (if mapper requested `DiscordInfo`)
    pub discord_info: Arc<dyn DiscordInfoProvider>,
}

/// TTS text transformation service using WASM-based mappers.
///
/// Storage-agnostic: the caller provides [`MapperRepository`] and [`PipelineRepository`]
/// implementations (e.g., Postgres-backed in production, in-memory for tests).
///
/// # Initialization
/// ```ignore
/// let service = TtsMatchingService::new(mapper_repo, pipeline_repo)?;
/// ```
pub struct TtsMatchingService {
    mapper_repo: Arc<dyn MapperRepository>,
    pipeline_repo: Arc<dyn PipelineRepository>,
    engine_state: Arc<EngineState>,
}

impl TtsMatchingService {
    /// Create a new TTS matching service from caller-provided repositories.
    pub fn new(
        mapper_repo: Arc<dyn MapperRepository>,
        pipeline_repo: Arc<dyn PipelineRepository>,
    ) -> Result<Self> {
        let engine_state = Arc::new(EngineState::new()?);
        Ok(Self {
            mapper_repo,
            pipeline_repo,
            engine_state,
        })
    }

    /// Get the mapper repository for CRUD operations.
    pub fn mapper_repo(&self) -> &dyn MapperRepository {
        self.mapper_repo.as_ref()
    }

    /// Get the pipeline repository for ordering and configuration.
    pub fn pipeline_repo(&self) -> &dyn PipelineRepository {
        self.pipeline_repo.as_ref()
    }

    /// Process text through the registered mapper pipeline.
    pub async fn process(&self, ctx: ProcessContext) -> Result<String> {
        let ordered_ids = self.pipeline_repo.get_ordered().await?;
        pipeline::execute_pipeline(
            ordered_ids,
            ctx,
            self.mapper_repo.as_ref(),
            Arc::clone(&self.engine_state),
        )
        .await
    }

    /// Process text through a specific list of mapper IDs, returning per-step trace results.
    pub async fn process_with_ids(
        &self,
        ctx: ProcessContext,
        mapper_ids: &[String],
    ) -> Result<(String, Vec<MapperStepResult>)> {
        pipeline::execute_pipeline_traced(
            mapper_ids.to_vec(),
            ctx,
            self.mapper_repo.as_ref(),
            Arc::clone(&self.engine_state),
        )
        .await
    }
}
