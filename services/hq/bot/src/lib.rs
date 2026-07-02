use ::serenity::all::GatewayIntents;
use hq_core::{
    service::{DiscordNameResolver, DiscordNameResolverSlot},
    PlaybackEvent, Service,
};
use poise::serenity_prelude as serenity;
use std::sync::Arc;
use tokio::sync::broadcast;

pub mod commands;
pub mod discord_resolver;
pub mod error;
pub mod events;
pub mod ui;
pub mod util;

use discord_resolver::SerenityNameResolver;
pub use error::BotError;

pub struct Data {
    pub service: Service,
}

pub type Error = BotError;
pub type Context<'a> = poise::Context<'a, Data, Error>;

async fn on_error(err: poise::FrameworkError<'_, Data, Error>) {
    match err {
        poise::FrameworkError::Command { error, ctx, .. } => {
            if error.is_internal() {
                tracing::error!("Internal bot error in command '{}': {:?}", ctx.command().name, error);
            }
            let embed = ui::embeds::error_embed(error.to_user_message().as_ref());
            let reply = poise::CreateReply::default().embed(embed).ephemeral(true);
            if let Err(e) = ctx.send(reply).await {
                tracing::error!("Failed to send error reply: {e:?}");
            }
        }
        other => {
            if let Err(e) = poise::builtins::on_error(other).await {
                tracing::error!("Unhandled framework error: {e:?}");
            }
        }
    }
}

/// Periodically refreshes the set of Zako worker-bot Discord user ids from
/// Traffic-Light's registry. Used by the voice-state auto-leave logic to detect
/// whether *any* Zako bot occupies a channel. On fetch failure the last-known set
/// is retained (never cleared) so a transient TL outage can't cause spurious leaves.
fn spawn_zako_bot_id_refresh(
    service: Arc<Service>,
    zako_bot_ids: Arc<tokio::sync::RwLock<std::collections::HashSet<serenity::UserId>>>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            match service.audio_engine.list_bot_ids().await {
                Ok(ids) => {
                    let parsed: std::collections::HashSet<serenity::UserId> = ids
                        .iter()
                        .filter_map(|id| id.parse::<u64>().ok())
                        .map(serenity::UserId::new)
                        .collect();
                    if parsed.is_empty() {
                        tracing::warn!("list_bot_ids returned no usable ids; keeping previous set");
                    } else {
                        *zako_bot_ids.write().await = parsed;
                    }
                }
                Err(e) => tracing::warn!("Failed to refresh Zako bot ids from TL: {e}"),
            }
        }
    });
}

pub async fn run(
    service: Service,
    resolver_slot: DiscordNameResolverSlot,
    event_tx: broadcast::Sender<PlaybackEvent>,
) -> anyhow::Result<()> {
    let token = service.config.discord_bot_token.clone();
    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILDS;

    let zako_bot_ids = Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new()));
    spawn_zako_bot_id_refresh(Arc::new(service.clone()), zako_bot_ids.clone());

    let voice_handler = events::VoiceStateHandler {
        voice_state_service: service.voice_state.clone(),
        service: Arc::new(service.clone()),
        event_tx,
        joining: Arc::new(dashmap::DashSet::new()),
        zako_bot_ids,
    };

    let message_handler = events::MessageCreateHandler {
        service: service.clone().into(),
    };

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                commands::ping(),
                commands::tap::tap(),
                commands::settings::settings(),
                commands::channel::channel(),
                commands::voice::join(),
                commands::voice::leave(),
                commands::music::play(),
                commands::music::stop(),
                commands::music::skip(),
                commands::music::volume(),
                commands::music::wedding(),
                commands::queue::queue(),
                commands::queue::clear(),
                commands::queue::enqueue(),
                commands::tts::tts(),
                commands::help::help(),
                commands::emoji_map::emoji_map(),
                commands::invites::invites(),
            ]
            .into_iter()
            .map(commands::tracing::with_tracing)
            .collect(),
            on_error: |err| Box::pin(on_error(err)),
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
        .event_handler(message_handler)
        .framework(framework)
        .await?;

    let cache_resolver = Arc::new(SerenityNameResolver {
        cache: client.cache.clone(),
        http: client.http.clone(),
    });
    let _ = resolver_slot.set(cache_resolver as Arc<dyn DiscordNameResolver>);

    client.start().await?;
    Ok(())
}
