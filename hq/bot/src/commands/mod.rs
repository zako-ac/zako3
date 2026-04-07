pub mod channel;
pub mod settings;
pub mod tap;

use crate::{Context, Error};

#[poise::command(slash_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Pong!").await?;
    Ok(())
}
