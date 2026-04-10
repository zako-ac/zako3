use hq_types::{hq::dtos::TapWithAccessDto, AudioMetadata, ChannelId, Track};
use poise::serenity_prelude::{self as serenity, Colour};

const THEME: Colour = Colour(0xeb3489);

pub fn settings_embed() -> serenity::CreateEmbed {
    serenity::CreateEmbed::new()
        .title("설정")
        .description("Zako3 설정을 관리하세요.")
        .colour(THEME)
}

pub fn tap_list_embed(taps: &[TapWithAccessDto]) -> serenity::CreateEmbed {
    let description = if taps.is_empty() {
        "아직 Tap이 없어요.".to_string()
    } else {
        taps.iter()
            .map(|t| {
                let lock = if t.has_access { "" } else { " 🔒" };
                format!("**{}**{lock} — {}", t.tap.name, t.tap.description)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    serenity::CreateEmbed::new()
        .title("내 Tap 목록")
        .description(description)
        .colour(THEME)
}

/// Embed shown after a track has been queued, once the audio engine confirms it.
pub fn track_queued_embed(track: &Track, position: usize) -> serenity::CreateEmbed {
    let title = track_title(track);
    let artist = track.metadatas.iter().find_map(|m| {
        if let AudioMetadata::Artist(a) = m { Some(a.as_str()) } else { None }
    });
    let image_url = track.metadatas.iter().find_map(|m| {
        if let AudioMetadata::ImageUrl(u) = m { Some(u.as_str()) } else { None }
    });

    let description = if position == 1 {
        "지금 재생 중".to_string()
    } else {
        format!("대기열 **{position}**번")
    };

    let mut embed = serenity::CreateEmbed::new()
        .title(title)
        .description(description)
        .colour(THEME);

    if let Some(artist) = artist {
        embed = embed.field("아티스트", artist, true);
    }
    if let Some(url) = image_url {
        embed = embed.thumbnail(url);
    }

    embed
}

pub fn queue_music_embed(tracks: &[Track]) -> serenity::CreateEmbed {
    let description = if tracks.is_empty() {
        "음악 대기열이 비어 있어요.".to_string()
    } else {
        tracks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let title = track_title(t);
                let paused = if t.paused { " ⏸" } else { "" };
                format!("{}. **{title}**{paused}", i + 1)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    serenity::CreateEmbed::new()
        .title("음악 대기열")
        .description(description)
        .colour(THEME)
}

pub fn queue_tts_embed(tracks: &[&Track]) -> serenity::CreateEmbed {
    let description = if tracks.is_empty() {
        "TTS 대기열이 비어 있어요.".to_string()
    } else {
        tracks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let text = t.request.audio_request.to_string();
                let preview = if text.len() > 80 {
                    format!("{}…", &text[..80])
                } else {
                    text
                };
                format!("{}. {preview}", i + 1)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    serenity::CreateEmbed::new()
        .title("TTS 대기열")
        .description(description)
        .colour(THEME)
}

fn track_title(track: &Track) -> String {
    track
        .metadatas
        .iter()
        .find_map(|m| {
            if let AudioMetadata::Title(t) = m {
                Some(t.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| track.request.audio_request.to_string())
}

pub fn channel_list_embed(channel_ids: &[ChannelId]) -> serenity::CreateEmbed {
    let description = if channel_ids.is_empty() {
        "이 서버에서 활성화된 TTS 채널이 없어요.".to_string()
    } else {
        channel_ids
            .iter()
            .map(|id| format!("<#{id}>"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    serenity::CreateEmbed::new()
        .title("TTS 채널")
        .description(description)
        .colour(THEME)
}

pub fn help_embed(category: HelpCategory) -> serenity::CreateEmbed {
    let (title, body) = match category {
        HelpCategory::Music => (
            "음악 명령어",
            "/play <검색어|URL> [소스] — 음악 검색 및 재생\n\
             /stop [범위] — 재생 정지 (`current` 또는 `queue`)\n\
             /skip [개수] — 트랙 건너뛰기\n\
             /volume <0-150> — 재생 볼륨 조절\n\
             /queue music — 현재 음악 대기열 보기\n\
             /queue web — 웹 대기열 인터페이스 열기\n\
             /clear [music|tts|all] — 대기열 비우기",
        ),
        HelpCategory::Tts => (
            "TTS 명령어",
            "/tts speak <메시지> [음성] — TTS 메시지 대기열에 추가\n\
             /tts stop [대상] — TTS 정지 (기본값: 본인; 타인은 음소거 권한 필요)\n\
             /tts skip [대상] — TTS 건너뛰기 (기본값: 본인; 타인은 음소거 권한 필요)\n\
             /voice [제공자] — TTS 음성 변경\n\
             /queue tts — 예정된 TTS 메시지 보기",
        ),
        HelpCategory::Admin => (
            "관리자 명령어",
            "/channel enable [채널] — 채널에서 봇 사용 허용\n\
             /channel disable [채널] — 채널에서 봇 사용 차단\n\
             /channel list — 활성화된 TTS 채널 목록 보기",
        ),
        HelpCategory::Overview => (
            "Zako3 도움말",
            "카테고리를 선택해 더 알아보세요:\n\n\
             🎵 **/help music** — 음악 재생 명령어\n\
             🎙️ **/help tts** — TTS 명령어\n\
             🛡️ **/help admin** — 채널 관리 명령어",
        ),
    };

    serenity::CreateEmbed::new()
        .title(title)
        .description(body)
        .colour(THEME)
}

/// Generic embed for commands that open a web page.
/// Pair with a link button using [`serenity::CreateButton::new_link`].
pub fn web_link_embed(title: &str, description: &str) -> serenity::CreateEmbed {
    serenity::CreateEmbed::new()
        .title(title)
        .description(description)
        .colour(THEME)
}

pub fn error_embed(message: &str) -> serenity::CreateEmbed {
    serenity::CreateEmbed::new()
        .title("오류")
        .description(message)
        .colour(Colour::RED)
}

#[derive(Debug, Clone, Copy)]
pub enum HelpCategory {
    Overview,
    Music,
    Tts,
    Admin,
}
