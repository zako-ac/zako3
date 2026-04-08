use crate::types::{ChannelInfo, UserInfo};

/// Query Discord channel information by channel ID
/// Only meaningful if the mapper declared DiscordInfo in its input_data
/// Returns None on non-wasm32 targets, when DiscordInfo is not available, or if the lookup fails
#[cfg(target_arch = "wasm32")]
pub fn query_channel_raw(channel_id: &str) -> Option<ChannelInfo> {
    unsafe {
        let len = query_channel(channel_id.as_ptr() as i32, channel_id.len() as i32);
        if len <= 0 {
            return None;
        }
        let mut buf = vec![0u8; len as usize];
        read_response(buf.as_mut_ptr() as i32);
        serde_json::from_slice(&buf).ok()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn query_channel_raw(_channel_id: &str) -> Option<ChannelInfo> {
    None
}

/// Query Discord user information by Discord user ID
/// Only meaningful if the mapper declared DiscordInfo in its input_data
/// Returns None on non-wasm32 targets, when DiscordInfo is not available, or if the lookup fails
#[cfg(target_arch = "wasm32")]
pub fn query_user_raw(user_id: &str) -> Option<UserInfo> {
    unsafe {
        let len = query_user(user_id.as_ptr() as i32, user_id.len() as i32);
        if len <= 0 {
            return None;
        }
        let mut buf = vec![0u8; len as usize];
        read_response(buf.as_mut_ptr() as i32);
        serde_json::from_slice(&buf).ok()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn query_user_raw(_user_id: &str) -> Option<UserInfo> {
    None
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "zako3")]
unsafe extern "C" {
    fn query_channel(channel_id_ptr: i32, channel_id_len: i32) -> i32;
    fn query_user(user_id_ptr: i32, user_id_len: i32) -> i32;
    fn read_response(dst_ptr: i32);
}
