use crate::{ui, Context, Error};
use poise::serenity_prelude as serenity;

#[poise::command(slash_command)]
pub async fn settings(ctx: Context<'_>) -> Result<(), Error> {
    let config = &ctx.data().service.config;
    let settings_url = format!("{}/settings", config.zako_website_url);
    let login_url = ctx.data().service.auth.get_login_url(Some("/settings"));

    let embed = ui::embeds::settings_embed();
    let open_button = serenity::CreateButton::new_link(&settings_url).label("Open");
    let login_button = serenity::CreateButton::new_link(&login_url).label("Login");
    let row = serenity::CreateActionRow::Buttons(vec![open_button, login_button]);

    ctx.send(
        poise::CreateReply::default()
            .embed(embed)
            .components(vec![row]),
    )
    .await?;

    Ok(())
}
