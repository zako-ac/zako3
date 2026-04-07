use std::sync::Arc;

use wasmtime::{Linker, Store, TypedFunc};

use crate::{
    service::DiscordInfoProvider,
    wasm::{host, input::WasmOutput},
    Result,
};

use super::host::StoreData;

pub(crate) fn run_mapper_sync(
    module: &wasmtime::Module,
    stdin_json: Vec<u8>,
    needs_discord_info: bool,
    rt_handle: tokio::runtime::Handle,
    discord_info: Arc<dyn DiscordInfoProvider>,
    guild_id: zako3_types::GuildId,
) -> Result<WasmOutput> {
    // Get engine from the module
    let engine = module.engine();

    // Build store with data
    let mut store = Store::new(
        engine,
        StoreData {
            response_buffer: Vec::new(),
            rt_handle: rt_handle.clone(),
            discord_info: discord_info.clone(),
            guild_id,
        },
    );

    // Build linker
    let mut linker = Linker::<StoreData>::new(engine);

    // Add discord host functions if needed
    if needs_discord_info {
        host::add_discord_host_funcs(&mut linker, rt_handle)?;
    }

    // Instantiate (module is already compiled)
    let instance = linker.instantiate(&mut store, module)?;

    // Get typed functions — fail fast if missing
    let alloc_fn: TypedFunc<(i32,), (i32,)> = instance
        .get_typed_func(&mut store, "alloc")
        .map_err(|_| wasmtime::Error::msg("wasm module missing required export: alloc"))?;

    let process_fn: TypedFunc<(i32, i32), (i64,)> = instance
        .get_typed_func(&mut store, "process")
        .map_err(|_| wasmtime::Error::msg("wasm module missing required export: process"))?;

    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| wasmtime::Error::msg("wasm module has no exported memory"))?;

    // Allocate in WASM memory + write input JSON
    let input_len = stdin_json.len() as i32;
    let (wasm_ptr,) = alloc_fn.call(&mut store, (input_len,))?;
    let wasm_ptr_usize = wasm_ptr as usize;
    memory.data_mut(&mut store)[wasm_ptr_usize..wasm_ptr_usize + stdin_json.len()]
        .copy_from_slice(&stdin_json);

    // Call process
    let (packed,) = process_fn.call(&mut store, (wasm_ptr, input_len))?;

    // Unpack output location (treat as u64 to avoid sign issues)
    let packed = packed as u64;
    let out_ptr = (packed >> 32) as usize;
    let out_len = (packed & 0xFFFF_FFFF) as usize;

    // Read output bytes
    let output_bytes = memory
        .data(&store)
        .get(out_ptr..out_ptr + out_len)
        .ok_or_else(|| wasmtime::Error::msg("wasm output pointer out of bounds"))?
        .to_vec();

    // Parse output
    let output: WasmOutput = serde_json::from_slice(&output_bytes)?;
    Ok(output)
}
