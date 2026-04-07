use serde::{Deserialize, Serialize};

/// Metadata for a registered WASM text transformation mapper.
///
/// A mapper is a WASM module that processes TTS text through a transformation pipeline.
/// It is identified by a unique `id`, stored on disk as a `.wasm` file, and verified
/// by SHA-256 hash to ensure integrity.
///
/// # Fields
/// - `id`: Unique identifier, used to reference the mapper in the pipeline
/// - `name`: Human-readable name for display purposes
/// - `wasm_filename`: Name of the `.wasm` file in the mapper directory (e.g., `"lowercase.wasm"`)
/// - `sha256_hash`: SHA-256 hash of the WASM file (hex-encoded), used for integrity verification
/// - `input_data`: List of input data types the mapper requires from the host
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmMapper {
    /// Unique identifier for this mapper
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// WASM filename in the mapper storage directory
    pub wasm_filename: String,
    /// SHA-256 hash (hex-encoded) of the WASM file for integrity checks
    pub sha256_hash: String,
    /// List of input data types this mapper requires (if any)
    pub input_data: Vec<MapperInputData>,
}

/// Input data types that a mapper can request from the host.
///
/// When a mapper declares these in its `input_data` list, the host provides the corresponding
/// data during execution.
///
/// # Variants
/// - `MappingList`: Text and emoji mapping rules that can be applied
/// - `DiscordInfo`: Guild and channel IDs, allowing the mapper to query Discord info
/// - `CallerInfo`: Discord user ID of the TTS requester
/// - `MapperList`: History of previous mappers in the pipeline and list of mappers to execute next
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MapperInputData {
    /// Text and emoji mapping rules
    MappingList,
    /// Guild and channel IDs for Discord queries
    DiscordInfo,
    /// Discord user ID of the TTS caller
    CallerInfo,
    /// Mapper pipeline execution history and future
    MapperList,
}

/// Discord channel information returned by Discord info queries.
///
/// Used when a WASM mapper calls `input.query_channel()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    /// Channel name
    pub name: String,
}

/// Discord user information returned by Discord info queries.
///
/// Used when a WASM mapper calls `input.query_user()`. Contains username and optional
/// guild-specific and global nicknames.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// Discord username (e.g., "alice")
    pub username: String,
    /// Global nickname if set
    pub global_nickname: Option<String>,
    /// Guild-specific nickname if set
    pub guild_nickname: Option<String>,
}

/// Summary of a mapper's execution in a pipeline run.
///
/// Provides a snapshot of which mappers ran and their success/failure status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapperSummary {
    /// Mapper ID
    pub id: String,
    /// Mapper name
    pub name: String,
    /// Whether this mapper executed successfully
    pub success: bool,
}

/// Detailed result of a single mapper step in a traced pipeline run.
///
/// Captures the text before and after the mapper executed, along with success status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapperStepResult {
    /// Mapper ID
    pub mapper_id: String,
    /// Mapper name
    pub mapper_name: String,
    /// Text fed into this mapper
    pub input_text: String,
    /// Text produced by this mapper (same as input_text if output was empty)
    pub output_text: String,
    /// Whether the mapper executed successfully
    pub success: bool,
    /// Mapper-reported error message, if any
    pub error: Option<String>,
}
