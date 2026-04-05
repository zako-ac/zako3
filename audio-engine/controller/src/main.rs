use std::sync::Arc;

use serenity::Client;
use serenity::all::GatewayIntents;
use songbird::SerenityInit;

use zako3_audio_engine_controller::{AudioEngineRpcServer, AudioEngineServer, config::AppConfig};

use zako3_audio_engine_core::engine::session_manager::SessionManager;
use zako3_audio_engine_infra::{
    discord::SongbirdDiscordService, state::RedisStateService, taphub::RealTapHubService,
};

use jsonrpsee::server::Server;
use tower::ServiceBuilder;
use tower_http::validate_request::ValidateRequestHeaderLayer;
use zako3_audio_engine_telemetry::TelemetryConfig;
use zako3_taphub_transport_client::{TransportClient, load_certs};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load();
    let addr = config.addr();
    let ae_token = config.ae_token.clone();

    println!("Starting zako3 audio engine (JSON-RPC) with token authentication...");

    let _ = rustls::crypto::ring::default_provider().install_default();

    let telem_config = TelemetryConfig {
        service_name: config.service_name.clone(),
        otlp_endpoint: config.otlp_endpoint.clone(),
        metrics_port: config.metrics_port,
    };

    let telemetry = zako3_audio_engine_telemetry::init(telem_config).await?;

    let intents = GatewayIntents::GUILD_VOICE_STATES;
    let mut client = Client::builder(&config.discord_token, intents)
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

    let certs = load_certs("cert.pem")?;

    let taphub_transport = TransportClient::new(
        "0.0.0.0:0".parse()?,
        "127.0.0.1:4000".parse()?,
        "localhost".to_string(),
        certs,
    )?;

    taphub_transport.connect().await?;

    let taphub_service = Arc::new(RealTapHubService::new(Arc::new(taphub_transport)));
    //let taphub_service = Arc::new(StubTapHubService);

    let discord_service = Arc::new(SongbirdDiscordService::new(songbird_manager));
    let state_service = Arc::new(RedisStateService::new(&config.redis_url)?);

    let session_manager = Arc::new(SessionManager::new(
        discord_service,
        state_service,
        taphub_service,
    ));

    tracing::info!("Audio Engine Server listening on {}", addr);

    let engine_server = AudioEngineServer::new(session_manager);

    telemetry.healthy();

    let middleware = ServiceBuilder::new().layer(ValidateRequestHeaderLayer::custom(
        move |request: &mut http::Request<jsonrpsee::server::HttpBody>| match request
            .headers()
            .get("X-AE-Token")
        {
            Some(token) if token == ae_token.as_bytes() => Ok(()),
            _ => {
                tracing::warn!("Unauthorized access attempt from {:?}", request.uri());
                Err(http::Response::builder()
                    .status(http::StatusCode::UNAUTHORIZED)
                    .body(jsonrpsee::server::HttpBody::default())
                    .unwrap())
            }
        },
    ));

    let server = Server::builder()
        .set_http_middleware(middleware)
        .build(addr)
        .await?;
    let handle = server.start(engine_server.into_rpc());

    handle.stopped().await;

    Ok(())
}
