use zako3_ae_proxy::run;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    run().await
}
