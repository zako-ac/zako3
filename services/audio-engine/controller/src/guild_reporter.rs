use std::time::Duration;

use zako3_types::GuildId;

/// Reports the current guild list to TL once. Fire-and-forget friendly.
pub async fn report_guilds_once(
    ctx: &serenity::all::Context,
    tl_rpc_url: &str,
    token: &str,
) {
    let guilds: Vec<GuildId> = ctx
        .cache
        .guilds()
        .into_iter()
        .map(|g: serenity::model::id::GuildId| GuildId::from(g.get()))
        .collect();

    tracing::debug!(guild_count = guilds.len(), "Reporting guild list to TL");

    match zako3_tl_client::TlClient::connect(tl_rpc_url).await {
        Ok(client) => {
            if let Err(e) = client.report_guilds(token.to_string(), guilds).await {
                tracing::warn!("guild_reporter: report_guilds failed: {e}");
            }
        }
        Err(e) => {
            tracing::warn!("guild_reporter: failed to connect to TL at {tl_rpc_url}: {e}");
        }
    }
}

/// Periodically reports the guild list to TL. Runs forever.
pub async fn run_guild_reporter(
    ctx: serenity::all::Context,
    tl_rpc_url: String,
    token: String,
) {
    loop {
        report_guilds_once(&ctx, &tl_rpc_url, &token).await;
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
