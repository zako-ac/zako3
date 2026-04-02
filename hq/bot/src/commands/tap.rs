use crate::{Context, Error};
use hq_types::hq::CreateTapDto;

#[poise::command(slash_command, subcommands("create", "list"))]
pub async fn tap(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
pub async fn create(
    ctx: Context<'_>,
    #[description = "Name of the tap"] name: String,
) -> Result<(), Error> {
    let service = &ctx.data().service;
    let discord_id = ctx.author().id.to_string();
    let username = &ctx.author().name;

    // Find or create user
    // We need to expose a method for this on Service or AuthService
    // For now assuming AuthService has get_or_create_user
    let user = service
        .auth
        .get_or_create_user(&discord_id, username)
        .await?;

    let dto = CreateTapDto {
        name,
        description: None,
    };
    let tap = service.tap.create(user.id.0, dto).await?;

    ctx.say(format!("Tap '{}' created! ID: {}", tap.name.0, tap.id.0))
        .await?;
    Ok(())
}

#[poise::command(slash_command)]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    let service = &ctx.data().service;
    let discord_id = ctx.author().id.to_string();
    let username = &ctx.author().name;

    let user = service
        .auth
        .get_or_create_user(&discord_id, username)
        .await?;
    let taps = service.tap.list_by_user(user.id.0).await?;

    let mut response = String::from("Your Taps:\n");
    for tap in taps {
        response.push_str(&format!("- {} (ID: {})\n", tap.name.0, tap.id.0));
    }

    ctx.say(response).await?;
    Ok(())
}
