use crate::{ui, ui::HelpCategory, Context, Error};

#[derive(Debug, poise::ChoiceParameter)]
pub enum HelpCategoryChoice {
    #[name = "Music"]
    Music,
    #[name = "TTS"]
    Tts,
    #[name = "Admin"]
    Admin,
}

/// Show help for bot commands.
#[poise::command(
    slash_command,
    name_localized("ko", "도움말"),
    description_localized("en-US", "Show help for bot commands"),
    description_localized("ko", "봇 명령어 도움말")
)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Command category"]
    #[description_localized("ko", "명령어 카테고리")]
    category: Option<HelpCategoryChoice>,
) -> Result<(), Error> {
    let cat = match category {
        None => HelpCategory::Overview,
        Some(HelpCategoryChoice::Music) => HelpCategory::Music,
        Some(HelpCategoryChoice::Tts) => HelpCategory::Tts,
        Some(HelpCategoryChoice::Admin) => HelpCategory::Admin,
    };

    let embed = ui::embeds::help_embed(cat);
    ctx.send(poise::CreateReply::default().embed(embed).ephemeral(true))
        .await?;
    Ok(())
}
