use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct EmojiInfo {
    pub id: String,
    pub name: String,
    pub animated: bool,
}

/// Extract all unique Discord custom emojis from message content, preserving order.
pub fn extract_custom_emojis(content: &str) -> Vec<EmojiInfo> {
    let mut result = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut remaining = content;

    while let Some(pos) = remaining.find('<') {
        let slice = &remaining[pos..];
        if let Some(emoji) = parse_discord_emoji(slice) {
            let token_len = 1
                + if emoji.animated { 2 } else { 1 }
                + emoji.name.len()
                + 1
                + emoji.id.len()
                + 1;
            if seen.insert(emoji.id.clone()) {
                result.push(emoji);
            }
            remaining = &remaining[pos + token_len..];
        } else {
            remaining = &remaining[pos + 1..];
        }
    }

    result
}

pub fn parse_discord_emoji(s: &str) -> Option<EmojiInfo> {
    let s = s.strip_prefix('<')?;
    let (animated, s) = if let Some(rest) = s.strip_prefix("a:") {
        (true, rest)
    } else if let Some(rest) = s.strip_prefix(':') {
        (false, rest)
    } else {
        return None;
    };

    let colon = s.find(':')?;
    let name = &s[..colon];
    let rest = &s[colon + 1..];
    let gt = rest.find('>')?;
    let id = &rest[..gt];

    if name.is_empty() || id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }

    Some(EmojiInfo {
        id: id.to_string(),
        name: name.to_string(),
        animated,
    })
}
