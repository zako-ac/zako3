use std::sync::Arc;

use wasmtime::{Caller, Linker};
use zako3_types::GuildId;

use crate::{service::DiscordInfoProvider, Result};

pub(crate) struct StoreData {
    pub response_buffer: Vec<u8>,
    pub rt_handle: tokio::runtime::Handle,
    pub discord_info: Arc<dyn DiscordInfoProvider>,
    #[allow(dead_code)]
    pub guild_id: GuildId,
}

pub(crate) fn add_discord_host_funcs(
    linker: &mut Linker<StoreData>,
    _rt_handle: tokio::runtime::Handle,
) -> Result<()> {
    linker.func_wrap(
        "zako3",
        "query_channel",
        |mut caller: Caller<StoreData>, channel_id_ptr: i32, channel_id_len: i32| -> i32 {
            let channel_id_ptr = channel_id_ptr as usize;
            let channel_id_len = channel_id_len as usize;

            // Read channel ID from WASM memory
            let memory = match caller.get_export("memory") {
                Some(wasmtime::Extern::Memory(m)) => m,
                _ => return 0,
            };

            let data = match memory.data(&caller).get(channel_id_ptr..channel_id_ptr + channel_id_len) {
                Some(d) => d,
                None => return 0,
            };

            let channel_id_str = match std::str::from_utf8(data) {
                Ok(s) => s,
                Err(_) => return 0,
            };

            let channel_id = match channel_id_str.parse::<u64>() {
                Ok(id) => zako3_types::ChannelId::from(id),
                Err(_) => return 0,
            };

            let store_data = caller.data_mut();
            let rt_handle = store_data.rt_handle.clone();
            let discord_info = Arc::clone(&store_data.discord_info);

            // Query channel info asynchronously
            if let Some(info) = rt_handle.block_on(discord_info.get_channel_info(channel_id))
                && let Ok(json) = serde_json::to_vec(&info) {
                store_data.response_buffer = json.clone();
                return json.len() as i32;
            }

            0
        },
    )?;

    linker.func_wrap(
        "zako3",
        "query_user",
        |mut caller: Caller<StoreData>, user_id_ptr: i32, user_id_len: i32| -> i32 {
            let user_id_ptr = user_id_ptr as usize;
            let user_id_len = user_id_len as usize;

            // Read user ID from WASM memory
            let memory = match caller.get_export("memory") {
                Some(wasmtime::Extern::Memory(m)) => m,
                _ => return 0,
            };

            let data = match memory.data(&caller).get(user_id_ptr..user_id_ptr + user_id_len) {
                Some(d) => d,
                None => return 0,
            };

            let user_id_str = match std::str::from_utf8(data) {
                Ok(s) => s,
                Err(_) => return 0,
            };

            let user_id = zako3_types::hq::user::DiscordUserId(user_id_str.to_string());

            let store_data = caller.data_mut();
            let rt_handle = store_data.rt_handle.clone();
            let discord_info = Arc::clone(&store_data.discord_info);

            // Query user info asynchronously
            if let Some(info) = rt_handle.block_on(discord_info.get_user_info(&user_id))
                && let Ok(json) = serde_json::to_vec(&info) {
                store_data.response_buffer = json.clone();
                return json.len() as i32;
            }

            0
        },
    )?;

    linker.func_wrap(
        "zako3",
        "read_response",
        |mut caller: Caller<StoreData>, dst_ptr: i32| {
            let dst_ptr = dst_ptr as usize;

            let response = {
                let store_data = caller.data_mut();
                std::mem::take(&mut store_data.response_buffer)
            };

            let memory = match caller.get_export("memory") {
                Some(wasmtime::Extern::Memory(m)) => m,
                _ => return,
            };

            // We need to drop the borrow from data_mut before accessing memory.data_mut
            let mem_data = memory.data_mut(&mut caller);
            if let Some(mem) = mem_data.get_mut(dst_ptr..dst_ptr.saturating_add(response.len())) {
                mem.copy_from_slice(&response);
            }
        },
    )?;

    Ok(())
}
