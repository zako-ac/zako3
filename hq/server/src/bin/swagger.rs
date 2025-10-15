use zako3_hq_server::controller::router::create_openapi_only_router;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    zako3_hq_server::util::tracing::init();

    tracing::info!("running Swagger UI at http://127.0.0.1:8012/swagger-ui");

    let router = create_openapi_only_router();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8012").await?;
    axum::serve(listener, router).await?;

    Ok(())
}
