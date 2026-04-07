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
//! 2. **Repository Layer** ([`MapperRepository`], [`PipelineRepository`]): Abstracts persistence
//!    of mapper metadata and pipeline configuration using SQLite.
//!
//! 3. **WASM Runtime** (wasm module): Executes WASM mappers using wasmtime, with support
//!    for Discord info lookups and text/emoji mapping rules.
//!
//! # Basic Usage
//!
//! ```ignore
//! use zako3_tts_matching::{TtsMatchingService, ProcessContext, WasmMapper};
//! use std::path::PathBuf;
//!
//! // Create service
//! let service = TtsMatchingService::new(
//!     PathBuf::from("/path/to/mappers"),     // where .wasm files are stored
//!     PathBuf::from("/path/to/database.db"), // SQLite DB for metadata
//! ).await?;
//!
//! // Register a mapper
//! let mapper = service.mapper_repo().create(WasmMapper {
//!     id: "lowercase".to_string(),
//!     name: "Lowercase".to_string(),
//!     wasm_filename: "lowercase.wasm".to_string(),
//!     sha256_hash: "abc123...".to_string(),
//!     input_data: vec![],
//! }).await?;
//!
//! // Configure pipeline execution order
//! service.pipeline_repo().set_ordered(&["lowercase"]).await?;
//!
//! // Process text
//! let ctx = ProcessContext {
//!     text: "Hello World".to_string(),
//!     // ... other fields
//! };
//! let result = service.process(ctx).await?;
//! assert_eq!(result, "hello world");
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

pub mod db;
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
