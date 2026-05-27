//! TTS text transformation pipeline using WASM-based mappers.
//!
//! This crate provides a framework for processing TTS text through a configurable pipeline
//! of WASM modules that can apply text transformations (e.g., "btw" → "by the way",
//! expanding abbreviations, converting text case, etc.).
//!
//! # Architecture
//!
//! The system consists of three main layers:
//!
//! 1. **Service Layer** ([`TtsMatchingService`]): The main entry point for processing text
//!    through the mapper pipeline.
//!
//! 2. **Repository Layer** ([`MapperRepository`], [`PipelineRepository`]): Storage-agnostic
//!    traits for persisting mapper metadata (with inline WASM bytes) and pipeline order.
//!    Implementations are provided by the consuming crate.
//!
//! 3. **WASM Runtime** (wasm module): Executes WASM mappers using wasmtime, with support
//!    for Discord info lookups and text/emoji mapping rules.
//!
//! # Basic Usage
//!
//! ```ignore
//! use std::sync::Arc;
//! use zako3_tts_matching::{TtsMatchingService, ProcessContext, WasmMapper};
//!
//! // Caller supplies repositories (e.g. Postgres-backed in production).
//! let service = TtsMatchingService::new(mapper_repo, pipeline_repo)?;
//!
//! let mapper = service.mapper_repo().create(WasmMapper {
//!     id: "lowercase".to_string(),
//!     name: "Lowercase".to_string(),
//!     wasm_bytes: std::fs::read("lowercase.wasm")?,
//!     sha256_hash: "abc123...".to_string(),
//!     input_data: vec![],
//! }).await?;
//!
//! service.pipeline_repo().set_ordered(&["lowercase".to_string()]).await?;
//!
//! let ctx = ProcessContext { /* ... */ };
//! let result = service.process(ctx).await?;
//! ```
//!
//! # Writing WASM Mappers
//!
//! Mappers are WASM modules compiled with the `zako3-tts-matching-sdk` library.
//! See the SDK documentation and examples for how to write mappers.
//!
//! # WASM ABI
//!
//! Mappers must export these functions:
//! - `alloc(size: i32) -> i32`: Allocate memory in WASM linear memory
//! - `process(input_ptr: i32, input_len: i32) -> i64`: Process input JSON, return packed output pointer/length
//! - `memory`: Exported linear memory for I/O

pub mod error;
pub mod model;
pub mod pipeline;
pub mod repo;
pub mod service;
pub mod wasm;

pub use error::{Error, Result};
pub use model::{ChannelInfo, MapperInputData, MapperStepResult, MapperSummary, UserInfo, WasmMapper};
pub use repo::{MapperRepository, PipelineRepository};
pub use service::{DiscordInfoProvider, ProcessContext, TtsMatchingService};
