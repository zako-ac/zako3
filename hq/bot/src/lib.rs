use hq_core::{
    Service,
    service::{DiscordNameResolver, DiscordNameResolverSlot},
};
use poise::serenity_prelude as serenity;
use std::sync::Arc;

pub mod commands;
pub mod discord_resolver;
pub mod events;
pub mod util;

use discord_resolver::SerenityNameResolver;

pub struct Data {
    pub service: Service,
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

pub async fn run(service: Service, resolver_slot: DiscordNameResolverSlot) -> anyhow::Result<()> {
    let token = service.config.discord_bot_token.clone();
    let intents = serenity::GatewayIntents::non_privileged()
        | serenity::GatewayIntents::GUILD_VOICE_STATES;

    let voice_handler = events::VoiceStateHandler {
        voice_state_service: service.voice_state.clone(),
    };

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![commands::ping(), commands::tap::tap(), commands::settings::settings()],
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    service: service.clone(),
                })
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(token, intents)
        .event_handler(voice_handler)
        .framework(framework)
        .await?;

    let cache_resolver = Arc::new(SerenityNameResolver { cache: client.cache.clone() });
    let _ = resolver_slot.set(cache_resolver as Arc<dyn DiscordNameResolver>);

    client.start().await?;
    Ok(())
}
