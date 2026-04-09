use std::sync::Arc;

use dashmap::DashMap;
use serenity::Client;
use serenity::all::GatewayIntents;
use sha2::{Digest, Sha256};
use songbird::SerenityInit;

use zako3_audio_engine_controller::{
    config::AppConfig, ready_waiter::create_ready_waiter, server::AudioEngineServer,
    voice_state::VoiceStateHandler,
};

use zako3_audio_engine_core::engine::session_manager::SessionManager;
use zako3_audio_engine_infra::{
    discord::SongbirdDiscordService, state::RedisStateService, taphub::RealTapHubService,
};

use zako3_audio_engine_telemetry::TelemetryConfig;
use zako3_taphub_transport_client::{TransportClient, load_certs};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load();
    let nats_url = config.nats_url.clone();

    println!("Starting zako3 audio engine (NATS)...");

    let _ = rustls::crypto::ring::default_provider().install_default();

    let telem_config = TelemetryConfig {
        service_name: config.service_name.clone(),
        otlp_endpoint: config.otlp_endpoint.clone(),
        metrics_port: config.metrics_port,
    };

    let telemetry = zako3_audio_engine_telemetry::init(telem_config).await?;

    let (ready_waiter, mut ready_recv) = create_ready_waiter();

    let ae_id = {
        let hash = Sha256::digest(config.discord_token.as_bytes());
        format!("{:x}", hash)[..16].to_string()
    };
    tracing::info!(ae_id, "Audio Engine starting");

    let certs = load_certs(&config.taphub_transport_cert_file).unwrap_or_else(|_| vec![]);

    let taphub_transport = match TransportClient::connect(
        "0.0.0.0:0".parse()?,
        &config.taphub_url,
        config.taphub_sni.clone(),
        certs,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("Failed to connect to taphub: {:?}", e);
            return Err(e.into());
        }
    };

    let taphub_service = Arc::new(RealTapHubService::new(Arc::new(taphub_transport)));
    let state_service = Arc::new(RedisStateService::new(&config.redis_url, ae_id).await?);

    // Pre-create Songbird so we can build the session manager before the Discord client
    let songbird_manager = songbird::Songbird::serenity();
    let discord_service = Arc::new(SongbirdDiscordService::new(songbird_manager.clone()));

    let session_manager = Arc::new(SessionManager::new(
        discord_service,
        state_service,
        taphub_service,
    ));

    let session_consumers = Arc::new(DashMap::new());

    let voice_state_handler = Arc::new(VoiceStateHandler {
        session_manager: session_manager.clone(),
        session_consumers: session_consumers.clone(),
    });

    let intents = GatewayIntents::GUILD_VOICE_STATES;
    let mut client = Client::builder(&config.discord_token, intents)
        .event_handler_arc(ready_waiter)
        .event_handler_arc(voice_state_handler)
        .register_songbird_with(songbird_manager)
        .await
        .expect("Err creating client");

    tokio::spawn(async move {
        let _ = client.start().await.map_err(|why| {
            tracing::error!("Client ended: {:?}", why);
            panic!();
        });
    });

    tracing::info!("Audio Engine connecting to NATS at {}", nats_url);

    let engine_server = Arc::new(AudioEngineServer::new(
        session_manager,
        nats_url,
        session_consumers,
    ));

    ready_recv.recv().await;

    tracing::info!("Audio Engine is ready and connected to Discord!");

    telemetry.healthy();

    // Start consuming
    if let Err(e) = engine_server.run().await {
        tracing::error!("RabbitMQ server error: {:?}", e);
        panic!();
    }

    Ok(())
}
