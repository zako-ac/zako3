use zako3_hq_server::{
    controller::router::create_router,
    core::app::{AppState, make_app_service},
    feature::config::load::load_config,
    infrastructure::{
        postgres::{connect_postgres, migrate_postgres},
        redis::RedisDb,
    },
};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    #[cfg(feature = "dotenv")]
    let _ = dotenvy::dotenv();

    zako3_hq_server::util::tracing::init();

    let config = load_config()?;

    tracing::info!(
        event = "boot",
        kind = "boot",
        message = "server started on address: {}",
        config.listen.http_bind_address
    );

    let postgres = connect_postgres(&config.infra.postgres_connection_string).await?;
    migrate_postgres(&postgres).await?;

    let redis = RedisDb::connect(&config.infra.redis_connection_string).await?;

    let app = AppState {
        config: config.clone(),
        service: make_app_service(config.clone(), postgres, redis).into(),
    };

    let router = create_router(app);

    let listener = tokio::net::TcpListener::bind(&config.listen.http_bind_address).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
