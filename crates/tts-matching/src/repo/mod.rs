use async_trait::async_trait;

use crate::{model::WasmMapper, Result};

pub mod mapper;
pub mod pipeline;

pub use mapper::SqliteMapperRepository;
pub use pipeline::SqlitePipelineRepository;

/// Repository for mapper metadata CRUD operations.
///
/// Manages the registration, updates, and removal of WASM mappers in the system.
/// All operations are async and backed by a SQLite database.
#[async_trait]
pub trait MapperRepository: Send + Sync {
    /// Register a new mapper and store its metadata.
    ///
    /// The mapper's `.wasm` file must already exist in the mapper directory.
    /// The `sha256_hash` is used during pipeline execution to verify file integrity.
    ///
    /// # Returns
    /// The created mapper with all fields populated, or an error if creation fails.
    async fn create(&self, mapper: WasmMapper) -> Result<WasmMapper>;

    /// Find a mapper by ID.
    ///
    /// # Returns
    /// The mapper if found, `None` if not registered, or an error on database failure.
    async fn find_by_id(&self, id: &str) -> Result<Option<WasmMapper>>;

    /// Update an existing mapper's metadata.
    ///
    /// # Returns
    /// The updated mapper, or an error if the mapper doesn't exist or update fails.
    async fn update(&self, mapper: WasmMapper) -> Result<WasmMapper>;

    /// Delete a mapper by ID.
    ///
    /// The WASM file is NOT automatically deleted from disk; only the metadata is removed.
    /// Returns success even if the mapper doesn't exist.
    async fn delete(&self, id: &str) -> Result<()>;

    /// List all registered mappers.
    async fn list_all(&self) -> Result<Vec<WasmMapper>>;
}

/// Repository for pipeline execution order configuration.
///
/// The pipeline order is a sequence of mapper IDs that determines which mappers
/// run and in what sequence. The order is persisted and can be updated at runtime.
#[async_trait]
pub trait PipelineRepository: Send + Sync {
    /// Get the current mapper execution order.
    ///
    /// # Returns
    /// A list of mapper IDs in execution order (empty list if no pipeline is configured).
    async fn get_ordered(&self) -> Result<Vec<String>>;

    /// Set the mapper execution order.
    ///
    /// Mappers in the list will execute in order. Any mapper IDs that don't exist
    /// in the mapper repository will be silently skipped during pipeline execution
    /// (logged as a warning).
    ///
    /// # Arguments
    /// `mapper_ids`: Ordered list of mapper IDs to execute. Pass an empty slice to disable all mappers.
    async fn set_ordered(&self, mapper_ids: &[String]) -> Result<()>;
}
