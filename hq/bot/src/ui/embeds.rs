use hq_types::{hq::dtos::TapWithAccessDto, AudioMetadata, ChannelId, Track};
use poise::serenity_prelude::{self as serenity, Colour};

const THEME: Colour = Colour(0xeb3489);

pub fn settings_embed() -> serenity::CreateEmbed {
    serenity::CreateEmbed::new()
        .title("Settings")
        .description("Manage your Zako3 settings.")
        .colour(THEME)
}

pub fn tap_list_embed(taps: &[TapWithAccessDto]) -> serenity::CreateEmbed {
    let description = if taps.is_empty() {
        "You have no Taps yet.".to_string()
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
        .title("Your Taps")
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
        "Now playing".to_string()
    } else {
        format!("Position **#{position}** in queue")
    };

    let mut embed = serenity::CreateEmbed::new()
        .title(title)
        .description(description)
        .colour(THEME);

    if let Some(artist) = artist {
        embed = embed.field("Artist", artist, true);
    }
    if let Some(url) = image_url {
        embed = embed.thumbnail(url);
    }

    embed
}

pub fn queue_music_embed(tracks: &[Track]) -> serenity::CreateEmbed {
    let description = if tracks.is_empty() {
        "The music queue is empty.".to_string()
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
        .title("Music Queue")
        .description(description)
        .colour(THEME)
}

pub fn queue_tts_embed(tracks: &[&Track]) -> serenity::CreateEmbed {
    let description = if tracks.is_empty() {
        "The TTS queue is empty.".to_string()
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
        .title("TTS Queue")
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
        "No TTS channels are enabled in this server.".to_string()
    } else {
        channel_ids
            .iter()
            .map(|id| format!("<#{id}>"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    serenity::CreateEmbed::new()
        .title("TTS Channels")
        .description(description)
        .colour(THEME)
}

pub fn help_embed(category: HelpCategory) -> serenity::CreateEmbed {
    let (title, body) = match category {
        HelpCategory::Music => (
            "Music Commands",
            "/play <query|url> [source] — Search and play audio\n\
             /stop [scope] — Stop playback (`current` or `queue`)\n\
             /skip [count] — Skip one or more tracks\n\
             /volume <0-150> — Adjust playback volume\n\
             /queue music — Show the current music queue\n\
             /queue web — Open the web queue interface\n\
             /clear [music|tts|all] — Clear a queue",
        ),
        HelpCategory::Tts => (
            "TTS Commands",
            "/tts speak <message> [voice] — Queue a TTS message\n\
             /tts stop [target] — Stop TTS (self by default; others requires Mute Members)\n\
             /tts skip [target] — Skip TTS (self by default; others requires Mute Members)\n\
             /voice [provider] — Change your TTS voice\n\
             /queue tts — Show upcoming TTS messages",
        ),
        HelpCategory::Admin => (
            "Admin Commands",
            "/channel enable [channel] — Allow bot use in a channel\n\
             /channel disable [channel] — Prevent bot use in a channel\n\
             /channel list — Show all enabled TTS channels",
        ),
        HelpCategory::Overview => (
            "Zako3 Help",
            "Choose a category to learn more:\n\n\
             🎵 **/help music** — Music playback commands\n\
             🎙️ **/help tts** — Text-to-speech commands\n\
             🛡️ **/help admin** — Channel management commands",
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
        .title("Error")
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
