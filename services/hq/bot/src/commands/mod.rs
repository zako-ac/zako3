pub mod channel;
pub mod emoji_map;
pub mod invites;
pub mod help;
pub mod music;
pub mod queue;
pub mod settings;
pub mod tap;
pub mod tracing;
pub mod tts;
pub mod voice;

use crate::{Context, Error};

#[poise::command(slash_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Pong!").await?;
    Ok(())
}
