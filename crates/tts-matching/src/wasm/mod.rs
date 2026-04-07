pub(crate) mod host;
pub(crate) mod input;
pub(crate) mod runner;

use std::collections::HashMap;
use std::sync::RwLock;

use crate::Result;

pub(crate) struct EngineState {
    pub engine: wasmtime::Engine,
    module_cache: RwLock<HashMap<String, wasmtime::Module>>,
}

impl EngineState {
    pub fn new() -> Result<Self> {
        let engine = wasmtime::Engine::default();
        Ok(Self {
            engine,
            module_cache: RwLock::new(HashMap::new()),
        })
    }

    /// Get or compile a WASM module, caching by SHA-256 hash.
    pub(crate) fn get_or_compile(
        &self,
        hash: &str,
        wasm_bytes: &[u8],
    ) -> Result<wasmtime::Module> {
        // Fast path: check read lock
        if let Ok(cache) = self.module_cache.read()
            && let Some(module) = cache.get(hash)
        {
            return Ok(module.clone());
        }

        // Slow path: compile and cache
        let module = wasmtime::Module::new(&self.engine, wasm_bytes)?;
        if let Ok(mut cache) = self.module_cache.write() {
            cache
                .entry(hash.to_string())
                .or_insert_with(|| module.clone());
        }
        Ok(module)
    }
}
