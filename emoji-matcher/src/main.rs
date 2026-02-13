use zako3_emoji_matcher::run;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    run().await
}
