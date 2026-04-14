use crate::{ui, Context, Error};

#[poise::command(slash_command, subcommands("list"))]
pub async fn tap(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    let service = &ctx.data().service;
    let discord_id = ctx.author().id.to_string();
    let username = &ctx.author().name;

    let user = service
        .auth
        .get_or_create_user(&discord_id, username, None, None, None)
        .await?;
    let taps = service.tap.list_by_user(user.id).await?;

    let embed = ui::embeds::tap_list_embed(&taps.data);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;

    Ok(())
}
