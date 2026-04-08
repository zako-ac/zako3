use std::collections::HashSet;
use std::time::Duration;

use hq_types::hq::settings::{EmojiMappingRule, PartialUserSettings, UserSettingsField};
use poise::serenity_prelude as serenity;

use crate::{Data, Error};

// ─── Emoji parsing ────────────────────────────────────────────────────────────

struct EmojiInfo {
    id: String,
    name: String,
    animated: bool,
}

/// Extract all unique Discord custom emojis from message content.
/// Returns at most the first 5 unique emojis (Discord modal limit).
fn extract_custom_emojis(content: &str) -> Vec<EmojiInfo> {
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
                if result.len() == 5 {
                    break;
                }
            }
            remaining = &remaining[pos + token_len..];
        } else {
            remaining = &remaining[pos + 1..];
        }
    }

    result
}

fn parse_discord_emoji(s: &str) -> Option<EmojiInfo> {
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

// ─── Modal ────────────────────────────────────────────────────────────────────

/// Modal for collecting emoji replacement text.
/// Metadata (id, name, animated) is encoded in each field's `custom_id`
/// as `em|{id}|{name}|{animated}` so it survives the round-trip.
pub struct EmojiMappingModal {
    pub fields: Vec<ParsedField>,
}

pub struct ParsedField {
    pub emoji_id: String,
    pub emoji_name: String,
    pub animated: bool,
    pub replacement: Option<String>,
}

impl EmojiMappingModal {
    fn from_emojis(emojis: &[EmojiInfo]) -> Self {
        Self {
            fields: emojis
                .iter()
                .map(|e| ParsedField {
                    emoji_id: e.id.clone(),
                    emoji_name: e.name.clone(),
                    animated: e.animated,
                    replacement: None,
                })
                .collect(),
        }
    }

}

impl poise::Modal for EmojiMappingModal {
    fn create(defaults: Option<Self>, custom_id: String) -> serenity::CreateInteractionResponse {
        let fields = defaults.map(|d| d.fields).unwrap_or_default();
        let components: Vec<serenity::CreateActionRow> = fields
            .iter()
            .map(|f| {
                let cid = format!("em|{}|{}|{}", f.emoji_id, f.emoji_name, if f.animated { '1' } else { '0' });
                serenity::CreateActionRow::InputText(
                    serenity::CreateInputText::new(
                        serenity::InputTextStyle::Short,
                        format!(":{}:", f.emoji_name),
                        cid,
                    )
                    .placeholder(format!(
                        "Replacement for :{}: (blank = skip)",
                        f.emoji_name
                    ))
                    .required(false),
                )
            })
            .collect();

        serenity::CreateInteractionResponse::Modal(
            serenity::CreateModal::new(custom_id, "Map Emoji Replacements")
                .components(components),
        )
    }

    fn parse(
        data: serenity::ModalInteractionData,
    ) -> Result<Self, &'static str> {
        let mut fields = Vec::new();

        for row in &data.components {
            for comp in &row.components {
                if let serenity::ActionRowComponent::InputText(input) = comp {
                    let parts: Vec<&str> = input.custom_id.splitn(4, '|').collect();
                    if parts.len() == 4 && parts[0] == "em" {
                        fields.push(ParsedField {
                            emoji_id: parts[1].to_string(),
                            emoji_name: parts[2].to_string(),
                            animated: parts[3] == "1",
                            replacement: input
                                .value
                                .as_deref()
                                .map(str::trim)
                                .filter(|v| !v.is_empty())
                                .map(str::to_string),
                        });
                    }
                }
            }
        }

        Ok(EmojiMappingModal { fields })
    }
}

// ─── Command ──────────────────────────────────────────────────────────────────

/// Right-click a message → Apps → "Map Emojis" to assign TTS replacement text
/// to any custom Discord emojis found in that message.
#[poise::command(context_menu_command = "Map Emojis")]
pub async fn emoji_map(
    ctx: poise::ApplicationContext<'_, Data, Error>,
    msg: serenity::Message,
) -> Result<(), Error> {
    let emojis = extract_custom_emojis(&msg.content);

    if emojis.is_empty() {
        ctx.send(
            poise::CreateReply::default()
                .content("No custom emojis found in this message.")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let user_id = ctx.interaction.user.id;
    let guild_id = ctx.interaction.guild_id.ok_or(Error::Forbidden)?;

    // Check guild-level permissions
    let member_perms = ctx
        .interaction
        .member
        .as_ref()
        .and_then(|m| m.permissions)
        .unwrap_or(serenity::Permissions::empty());
    let is_manage_guild = member_perms.contains(serenity::Permissions::MANAGE_GUILD);

    // Look up the registered user early — used for both the admin check and saving settings.
    let discord_id_str = user_id.get().to_string();
    let hq_user = ctx
        .data()
        .service
        .tap
        .get_user_by_discord_id(&discord_id_str)
        .await?;
    let is_bot_admin = hq_user
        .as_ref()
        .is_some_and(|u| u.permissions.contains(&"admin".to_string()));

    // Build scope select menu options
    let mut options = vec![
        serenity::CreateSelectMenuOption::new("Me for This Guild", "guild_user"),
        serenity::CreateSelectMenuOption::new("All Guilds (User)", "user"),
    ];
    if is_manage_guild {
        options.push(serenity::CreateSelectMenuOption::new(
            "Everyone in This Guild",
            "guild",
        ));
    }
    if is_bot_admin {
        options.push(serenity::CreateSelectMenuOption::new("Global", "global"));
    }

    let select = serenity::CreateSelectMenu::new(
        "emoji_map_scope",
        serenity::CreateSelectMenuKind::String { options },
    )
    .placeholder("Choose apply range…");

    // Step 1: Acknowledge the context-menu interaction with the scope selector
    let reply = ctx
        .send(
            poise::CreateReply::default()
                .content(format!(
                    "Found **{}** custom emoji(s). Choose the apply range:",
                    emojis.len()
                ))
                .components(vec![serenity::CreateActionRow::SelectMenu(select)])
                .ephemeral(true),
        )
        .await?;

    let reply_msg = reply.message().await?;

    // Step 2: Wait for scope selection
    let Some(scope_mci) = serenity::ComponentInteractionCollector::new(ctx.serenity_context())
        .message_id(reply_msg.id)
        .author_id(user_id)
        .timeout(Duration::from_secs(60))
        .await
    else {
        return Ok(());
    };

    let scope = match &scope_mci.data.kind {
        serenity::ComponentInteractionDataKind::StringSelect { values } => {
            values.first().cloned().unwrap_or_default()
        }
        _ => return Err(Error::Other("Unexpected component type".into())),
    };

    // Step 3: Show modal (responds to the scope select interaction)
    let modal_result = poise::execute_modal_on_component_interaction::<EmojiMappingModal>(
        ctx,
        scope_mci,
        Some(EmojiMappingModal::from_emojis(&emojis)),
        Some(Duration::from_secs(300)),
    )
    .await?;

    let Some(modal_data) = modal_result else {
        return Ok(());
    };

    // Step 4: Build emoji mapping rules from submitted values
    let rules: Vec<EmojiMappingRule> = modal_data
        .fields
        .into_iter()
        .filter_map(|f| {
            let replacement = f.replacement?;
            let ext = if f.animated { "gif" } else { "png" };
            Some(EmojiMappingRule {
                emoji_id: f.emoji_id.clone(),
                emoji_name: f.emoji_name,
                emoji_image_url: format!(
                    "https://cdn.discordapp.com/emojis/{}.{}",
                    f.emoji_id, ext
                ),
                replacement,
            })
        })
        .collect();

    if rules.is_empty() {
        ctx.send(
            poise::CreateReply::default()
                .content("No replacements were entered — nothing was saved.")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    // Step 5: Save to the selected scope
    let service = &ctx.data().service;
    let guild_id_str = guild_id.get().to_string();
    let partial = PartialUserSettings {
        emoji_mappings: UserSettingsField::Normal(rules.clone()),
        ..PartialUserSettings::empty()
    };

    match scope.as_str() {
        "guild_user" | "user" => {
            let hq_user = hq_user.ok_or(Error::Unauthorized)?;

            if scope == "guild_user" {
                service
                    .user_settings
                    .save_guild_user_settings(&hq_user.id, &guild_id_str, partial)
                    .await?;
            } else {
                service
                    .user_settings
                    .save_settings(hq_user.id, partial)
                    .await?;
            }
        }
        "guild" => {
            service
                .user_settings
                .save_guild_settings(&guild_id_str, partial)
                .await?;
        }
        "global" => {
            service.user_settings.save_global_settings(partial).await?;
        }
        _ => {}
    }

    // Step 6: Confirm via followup on original interaction token
    ctx.send(
        poise::CreateReply::default()
            .content(format!(
                "Saved **{}** emoji mapping(s) to **{}** scope.",
                rules.len(),
                scope_label(&scope),
            ))
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

fn scope_label(scope: &str) -> &str {
    match scope {
        "guild_user" => "Me for This Guild",
        "user" => "All Guilds",
        "guild" => "Everyone in This Guild",
        "global" => "Global",
        _ => scope,
    }
}
