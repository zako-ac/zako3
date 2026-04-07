use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Input data types that a WASM mapper can request from the host.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MapperInputData {
    MappingList,
    DiscordInfo,
    CallerInfo,
    MapperList,
}

/// A registered WASM text transformation mapper.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub struct WasmMapperDto {
    pub id: String,
    pub name: String,
    pub wasm_filename: String,
    pub sha256_hash: String,
    pub input_data: Vec<MapperInputData>,
}

/// Update mapper metadata (name and input data requirements).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub struct UpdateMapperDto {
    pub name: String,
    pub input_data: Vec<MapperInputData>,
}

/// Ordered list of mapper IDs forming the TTS pipeline.
/// Only mappers in this list are active; order determines execution sequence.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub struct PipelineOrderDto {
    pub mapper_ids: Vec<String>,
}

/// Result of a single mapper step in a traced pipeline evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub struct MapperStepResultDto {
    pub mapper_id: String,
    pub mapper_name: String,
    pub input_text: String,
    pub output_text: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Result of a full pipeline or single-mapper evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub struct EvaluateResultDto {
    pub final_text: String,
    pub steps: Vec<MapperStepResultDto>,
}

/// Request body for evaluating a specific set of mappers.
/// `mapper_ids` can be an unsaved local pipeline order.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub struct EvaluateRequestDto {
    pub text: String,
    pub mapper_ids: Vec<String>,
}

/// Request body for testing a single mapper in isolation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, zod_gen_derive::ZodSchema)]
pub struct EvaluateSingleRequestDto {
    pub text: String,
}
