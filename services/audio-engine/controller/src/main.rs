use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serenity::Client;
use serenity::all::GatewayIntents;
use songbird::SerenityInit;

use zako3_audio_engine_controller::{
    config::AppConfig,
    guild_reporter::{report_guilds_once, run_guild_reporter},
    ready_waiter::create_ready_waiter,
    server::AeTransportHandler,
};

use zako3_audio_engine_core::engine::session_manager::SessionManager;
use zako3_audio_engine_infra::{
    InMemoryStateService, discord::SongbirdDiscordService, taphub::RealTapHubService,
};

use zako3_telemetry::TelemetryConfig;
use zako3_ae_transport::TlClient;
use zako3_taphub_transport_client::{TransportClient, load_certs};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load();

    println!("Starting zako3 audio engine...");

    let _ = rustls::crypto::ring::default_provider().install_default();

    let telem_config = TelemetryConfig {
        service_name: config.service_name.clone(),
        otlp_endpoint: config.otlp_endpoint.clone(),
        metrics_port: Some(config.metrics_port),
    };

    let telemetry = zako3_telemetry::init(telem_config).await?;

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
    let state_service = Arc::new(InMemoryStateService::new());

    let addr = config.ae_transport_addr.clone();

    // Step 1: Connect to TL to receive our assigned Discord token.
    tracing::info!("Connecting to TL server at {} to receive Discord token", addr);
    let (token, _, connected) = loop {
        match TlClient::connect(addr.as_str(), HashMap::new()).await {
            Ok(result) => break result,
            Err(e) => {
                tracing::warn!("Failed to connect to TL server: {e}, retrying in 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    };
    tracing::info!("Received Discord token from TL server");

    // Step 2: Build the Discord / audio infrastructure with the assigned token.
    let songbird_manager = songbird::Songbird::serenity();
    let (ready_waiter, mut ready_recv, mut ctx_recv) = create_ready_waiter();

    let intents = GatewayIntents::GUILD_VOICE_STATES | GatewayIntents::GUILDS;
    let mut discord_client = Client::builder(&token.0, intents)
        .event_handler_arc(ready_waiter)
        .register_songbird_with(songbird_manager.clone())
        .await
        .expect("Failed to create Discord client");

    tokio::spawn(async move {
        if let Err(e) = discord_client.start().await {
            tracing::error!("Discord client ended: {:?}", e);
        } else {
            tracing::warn!("Discord client exited without error");
        }
        tracing::error!("Discord task terminated, shutting down AE process for restart");
        std::process::exit(1);
    });

    ready_recv.recv().await;
    let serenity_ctx = ctx_recv.recv().await.expect("ctx channel closed before ready");

    let discord_service = Arc::new(SongbirdDiscordService::new(songbird_manager.clone(), serenity_ctx.cache.clone()));

    let session_manager = Arc::new(SessionManager::new(
        discord_service,
        state_service,
        taphub_service,
    ));

    // Note: voice_state_handler would normally be registered with the Discord client,
    // but since we need the serenity context (and cache) before constructing it,
    // and the client is already running, we skip the voice_state_handler registration.
    // Voice state updates are still handled via the gateway (serenity's built-in handling).

    tracing::info!("Audio Engine is ready and connected to Discord!");
    telemetry.healthy();

    // Step 3b: Spawn background guild reporter.
    tokio::spawn(run_guild_reporter(
        serenity_ctx.clone(),
        config.tl_rpc_url.clone(),
        token.0.clone(),
    ));

    // Step 4: Serve requests. On TL disconnect, reconnect TL only (Discord stays alive).
    report_guilds_once(&serenity_ctx, &config.tl_rpc_url, &token.0).await;
    let handler = AeTransportHandler::new(session_manager.clone());
    if let Err(e) = connected.serve(handler).await {
        tracing::warn!("TL connection lost: {e}, reconnecting...");
    }

    // Step 5: Reconnect loop — TL only, Discord client continues running.
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        match TlClient::connect(addr.as_str(), HashMap::new()).await {
            Ok((_, _, connected)) => {
                tracing::info!("Reconnected to TL server");
                report_guilds_once(&serenity_ctx, &config.tl_rpc_url, &token.0).await;
                let handler = AeTransportHandler::new(session_manager.clone());
                if let Err(e) = connected.serve(handler).await {
                    tracing::warn!("TL connection lost: {e}, reconnecting...");
                }
            }
            Err(e) => {
                tracing::warn!("Failed to reconnect to TL: {e}");
            }
        }
    }
}
