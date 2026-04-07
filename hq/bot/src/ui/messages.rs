use poise::serenity_prelude::ChannelId;

pub fn channel_enabled(channel_id: ChannelId) -> String {
    format!("TTS enabled in <#{channel_id}>.")
}

pub fn channel_disabled(channel_id: ChannelId) -> String {
    format!("TTS disabled in <#{channel_id}>.")
}

pub fn bot_joined(channel_name: &str) -> String {
    format!("Joined **{channel_name}**.")
}

pub fn bot_left() -> &'static str {
    "Left the voice channel."
}

pub fn bot_moved(channel_name: &str) -> String {
    format!("Moved to **{channel_name}**.")
}

pub fn volume_set(level: u8) -> String {
    format!("Volume set to **{level}**.")
}

pub fn skipped(count: u32) -> String {
    if count == 1 {
        "Skipped the current track.".to_string()
    } else {
        format!("Skipped **{count}** tracks.")
    }
}

pub fn tts_skipped() -> &'static str {
    "Skipped the current TTS message."
}

pub fn tts_stopped() -> &'static str {
    "Stopped TTS playback."
}

pub fn tts_queued(preview: &str) -> String {
    format!("Queued: *{preview}*")
}

pub fn voice_changed(voice_name: &str) -> String {
    format!("Your TTS voice has been set to **{voice_name}**.")
}

pub fn cleared(queue_type: &str) -> String {
    format!("Cleared the **{queue_type}** queue.")
}

pub fn playback_stopped() -> &'static str {
    "Stopped playback."
}

pub fn queue_web_url(base_url: &str) -> String {
    format!("{base_url}/queue")
}
