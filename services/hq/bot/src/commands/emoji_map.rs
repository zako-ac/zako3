use std::time::Duration;

use hq_types::hq::settings::{EmojiMappingRule, PartialUserSettings, UserSettingsField};
use poise::serenity_prelude as serenity;

use crate::util::{EmojiInfo, extract_custom_emojis};
use crate::{Data, Error};

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
    // Discord modals allow at most 5 input rows.
    let mut emojis = extract_custom_emojis(&msg.content);
    emojis.truncate(5);

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

    // Step 5: Save to the selected scope.
    // Read-modify-write: the scope save path overwrites the whole settings blob, so we
    // must fetch the existing partial and replace only `emoji_mappings`, otherwise every
    // other setting at that scope would be wiped.
    let service = &ctx.data().service;
    let guild_id_str = guild_id.get().to_string();

    match scope.as_str() {
        "guild_user" => {
            let hq_user = hq_user.ok_or(Error::Unauthorized)?;
            let mut current = service
                .user_settings
                .get_guild_user_settings(&hq_user.id, &guild_id_str)
                .await?
                .unwrap_or_else(PartialUserSettings::empty);
            current.emoji_mappings = merge_emoji_rules(&current.emoji_mappings, rules.clone());
            service
                .user_settings
                .save_guild_user_settings(&hq_user.id, &guild_id_str, current)
                .await?;
        }
        "user" => {
            let hq_user = hq_user.ok_or(Error::Unauthorized)?;
            let mut current = service
                .user_settings
                .get_settings(hq_user.id.clone())
                .await?;
            current.emoji_mappings = merge_emoji_rules(&current.emoji_mappings, rules.clone());
            service
                .user_settings
                .save_settings(hq_user.id, current)
                .await?;
        }
        "guild" => {
            let mut current = service
                .user_settings
                .get_guild_settings(&guild_id_str)
                .await?
                .unwrap_or_else(PartialUserSettings::empty);
            current.emoji_mappings = merge_emoji_rules(&current.emoji_mappings, rules.clone());
            service
                .user_settings
                .save_guild_settings(&guild_id_str, current)
                .await?;
        }
        "global" => {
            let mut current = service
                .user_settings
                .get_global_settings()
                .await?
                .unwrap_or_else(PartialUserSettings::empty);
            current.emoji_mappings = merge_emoji_rules(&current.emoji_mappings, rules.clone());
            service.user_settings.save_global_settings(current).await?;
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

/// Merge freshly-submitted emoji rules into whatever is already stored at a scope.
/// New rules win on `emoji_id` conflicts; previously-mapped emojis not in this batch
/// are preserved. The existing field's `Important`/`Normal` wrapper is retained so the
/// cascade priority of an admin-set list isn't silently downgraded.
fn merge_emoji_rules(
    existing: &UserSettingsField<Vec<EmojiMappingRule>>,
    new_rules: Vec<EmojiMappingRule>,
) -> UserSettingsField<Vec<EmojiMappingRule>> {
    let (mut merged, important) = match existing {
        UserSettingsField::None => (Vec::new(), false),
        UserSettingsField::Normal(v) => (v.clone(), false),
        UserSettingsField::Important(v) => (v.clone(), true),
    };

    let new_ids: std::collections::HashSet<&str> =
        new_rules.iter().map(|r| r.emoji_id.as_str()).collect();
    merged.retain(|r| !new_ids.contains(r.emoji_id.as_str()));
    merged.extend(new_rules);

    if important {
        UserSettingsField::Important(merged)
    } else {
        UserSettingsField::Normal(merged)
    }
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
