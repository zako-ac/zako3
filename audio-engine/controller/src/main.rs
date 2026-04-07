use std::sync::Arc;

use serenity::Client;
use serenity::all::GatewayIntents;
use songbird::SerenityInit;

use zako3_audio_engine_controller::{
    config::AppConfig, ready_waiter::create_ready_waiter, server::AudioEngineServer,
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

    let intents = GatewayIntents::GUILD_VOICE_STATES;
    let mut client = Client::builder(&config.discord_token, intents)
        .event_handler_arc(ready_waiter)
        .register_songbird()
        .await
        .expect("Err creating client");

    let songbird_manager = {
        let data = client.data.read().await;
        data.get::<songbird::SongbirdKey>()
            .expect("Songbird VoiceClient placed in at initialisation.")
            .clone()
    };

    tokio::spawn(async move {
        let _ = client.start().await.map_err(|why| {
            tracing::error!("Client ended: {:?}", why);
            panic!();
        });
    });

    let certs = load_certs("cert.pem").unwrap_or_else(|_| vec![]);

    let taphub_transport = TransportClient::new(
        "0.0.0.0:0".parse()?,
        "127.0.0.1:4000".parse()?,
        "localhost".to_string(),
        certs,
    )?;

    if let Err(e) = taphub_transport.connect().await {
        tracing::warn!("Failed to connect to taphub: {:?}", e);
    }

    let taphub_service = Arc::new(RealTapHubService::new(Arc::new(taphub_transport)));

    let discord_service = Arc::new(SongbirdDiscordService::new(songbird_manager));
    let state_service = Arc::new(RedisStateService::new(&config.redis_url).await?);

    let session_manager = Arc::new(SessionManager::new(
        discord_service,
        state_service,
        taphub_service,
    ));

    tracing::info!("Audio Engine connecting to NATS at {}", nats_url);

    let engine_server = Arc::new(AudioEngineServer::new(session_manager, nats_url));

    ready_recv.recv().await;

    tracing::info!("Audio Engine is ready and connected to Discord!");

    telemetry.healthy();

    // Start consuming
    if let Err(e) = engine_server.run().await {
        tracing::error!("RabbitMQ server error: {:?}", e);
    }

    Ok(())
}
