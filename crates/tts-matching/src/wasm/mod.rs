pub(crate) mod host;
pub(crate) mod input;
pub(crate) mod runner;

use crate::Result;

pub(crate) struct EngineState {
    pub engine: wasmtime::Engine,
}

impl EngineState {
    pub fn new() -> Result<Self> {
        let engine = wasmtime::Engine::default();
        Ok(Self { engine })
    }
}
