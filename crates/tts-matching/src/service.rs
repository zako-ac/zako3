use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use zako3_types::{hq::settings::{EmojiMappingRule, TextMappingRule}, hq::user::DiscordUserId, ChannelId, GuildId};

use crate::{
    db::Db, model::ChannelInfo, model::UserInfo, pipeline, repo::{MapperRepository, PipelineRepository, SqliteMapperRepository, SqlitePipelineRepository}, wasm::EngineState, Result,
};

/// Provides Discord channel and user information to WASM mappers.
///
/// Mappers can optionally request Discord info by declaring `DiscordInfo` in their input data.
/// When a mapper queries Discord info, the host calls these methods to retrieve the data.
///
/// # Example
/// ```ignore
/// struct MyDiscordProvider {
///     // Your Discord client or API wrapper
/// }
///
/// #[async_trait]
/// impl DiscordInfoProvider for MyDiscordProvider {
///     async fn get_channel_info(&self, channel_id: ChannelId) -> Option<ChannelInfo> {
///         // Look up channel from Discord API
///     }
///
///     async fn get_user_info(&self, user_id: &DiscordUserId) -> Option<UserInfo> {
///         // Look up user from Discord API
///     }
/// }
/// ```
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
///
/// # Fields
/// - `text`: The TTS text to transform
/// - `guild_id`: Discord guild where TTS will be played
/// - `channel_id`: Discord channel where TTS will be played
/// - `caller`: Discord user ID of the TTS requester
/// - `text_mappings`: Text replacement rules to apply
/// - `emoji_mappings`: Emoji replacement rules to apply
/// - `discord_info`: Provider for Discord channel/user queries
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
/// This service manages a pipeline of WASM modules that transform TTS text.
/// Mappers are registered with metadata and stored as compiled WASM files.
/// The pipeline execution order is persisted and configurable.
///
/// # Initialization
/// ```ignore
/// let service = TtsMatchingService::new(
///     PathBuf::from("/path/to/mappers"),  // where .wasm files are stored
///     PathBuf::from("/path/to/db.sqlite"), // SQLite database for metadata
/// ).await?;
/// ```
///
/// # Mapper CRUD
/// Mappers are managed via the `mapper_repo()`:
/// ```ignore
/// let mapper = service.mapper_repo().create(WasmMapper { /* ... */ }).await?;
/// let updated = service.mapper_repo().update(mapper).await?;
/// service.mapper_repo().delete(&mapper.id).await?;
/// ```
///
/// # Pipeline Configuration
/// Set the execution order via `pipeline_repo()`:
/// ```ignore
/// service.pipeline_repo().set_ordered(&["lowercase", "expand_abbreviations"]).await?;
/// ```
///
/// # Processing
/// Transform text by executing the registered mappers in order:
/// ```ignore
/// let ctx = ProcessContext { /* ... */ };
/// let result = service.process(ctx).await?;
/// ```
pub struct TtsMatchingService {
    wasm_dir: PathBuf,
    mapper_repo: Arc<SqliteMapperRepository>,
    pipeline_repo: Arc<SqlitePipelineRepository>,
    engine_state: Arc<EngineState>,
}

impl TtsMatchingService {
    /// Create a new TTS matching service.
    ///
    /// # Arguments
    /// - `wasm_dir`: Directory where WASM mapper files are stored
    /// - `db_path`: SQLite database path for mapper metadata and pipeline configuration
    ///
    /// # Returns
    /// A new service instance, or an error if database initialization fails.
    pub async fn new(wasm_dir: PathBuf, db_path: PathBuf) -> Result<Self> {
        let db = Db::open(db_path).await?;
        let engine_state = Arc::new(EngineState::new()?);
        let mapper_repo = Arc::new(SqliteMapperRepository::new(db.clone()));
        let pipeline_repo = Arc::new(SqlitePipelineRepository::new(db));

        Ok(Self {
            wasm_dir,
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
    ///
    /// Executes mappers in the order configured by `pipeline_repo().set_ordered()`.
    /// Each mapper receives the transformed text from the previous mapper.
    ///
    /// # Returns
    /// The transformed text after all mappers complete, or an error if any mapper fails.
    pub async fn process(&self, ctx: ProcessContext) -> Result<String> {
        let ordered_ids = self.pipeline_repo.get_ordered().await?;
        pipeline::execute_pipeline(
            ordered_ids,
            ctx,
            self.wasm_dir.as_path(),
            self.mapper_repo.as_ref(),
            Arc::clone(&self.engine_state),
        )
        .await
    }
}
