use crate::{Context, Error};

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
        .get_or_create_user(&discord_id, username, None, None)
        .await?;
    let taps = service.tap.list_by_user(user.id).await?;

    let mut response = String::from("Your Taps:\n");
    for tap_item in taps.data {
        response.push_str(&format!(
            "- {} (ID: {})\n",
            tap_item.tap.name, tap_item.tap.id
        ));
    }

    ctx.say(response).await?;
    Ok(())
}
