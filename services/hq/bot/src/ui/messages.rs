use poise::serenity_prelude::ChannelId;

pub fn channel_enabled(channel_id: ChannelId) -> String {
    format!("앞으로 <#{channel_id}>의 메시지를 읽을게요!")
}

pub fn channel_disabled(channel_id: ChannelId) -> String {
    format!("앞으로 <#{channel_id}>의 메시지를 읽지 않을게요!")
}

pub fn bot_joined(channel_id: ChannelId) -> String {
    format!("<#{channel_id}>에 들어왔어요!")
}

pub fn bot_left() -> &'static str {
    "음성 채널에서 나왔어요."
}

pub fn bot_moved(channel_id: ChannelId) -> String {
    format!("음성 채널을 <#{channel_id}>로 옮겼어요!")
}

pub fn volume_set(level: u8) -> String {
    format!("음량을 **{level}%**로 설정할게요.")
}

pub fn skipped(count: u32) -> String {
    if count == 1 {
        "지금 재생 중인 트랙을 건너뛰었어요.".to_string()
    } else {
        format!("{count}개의 트랙을 건너뛰었어요.")
    }
}

pub fn tts_skipped() -> &'static str {
    "지금 재생 중인 TTS를 건너뛰었어요."
}

pub fn tts_stopped() -> &'static str {
    "지금 재생 중인 TTS를 멈췄어요."
}

pub fn tts_queued(preview: &str) -> String {
    format!("TTS 메시지를 큐에 추가했어요.\n```{preview}```")
}

pub fn voice_changed(voice_name: &str) -> String {
    format!("TTS 음성을 **{voice_name}**으로 변경했어요.")
}

pub fn cleared(queue_type: &str) -> String {
    format!("**{queue_type}** 큐를 비웠어요.")
}

pub fn playback_stopped() -> &'static str {
    "재생을 멈췄어요."
}

pub fn queue_web_url(base_url: &str) -> String {
    format!("{base_url}/queue")
}
