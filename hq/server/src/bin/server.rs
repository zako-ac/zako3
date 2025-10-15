use zako3_hq_server::{
    controller::router::create_router,
    core::{app::AppState, config::load_config},
    infrastructure::{
        postgres::{connect_postgres, migrate_postgres},
        redis::RedisDb,
    },
};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    zako3_hq_server::util::tracing::init();

    let config = load_config()?;

    tracing::info!(
        event = "boot",
        kind = "boot",
        message = "server started on address: {}",
        config.http_bind_address
    );

    let db = connect_postgres(&config.postgres_connection_string).await?;
    migrate_postgres(&db).await?;

    let redis = RedisDb::connect(&config.redis_connection_string).await?;

    let app = AppState {
        config: config.clone(),
        db,
        redis,
    };

    let router = create_router(app);

    let listener = tokio::net::TcpListener::bind(&config.http_bind_address).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
