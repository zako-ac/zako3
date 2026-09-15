//! This engine's side of the HQ control plane.
//!
//! An engine has to do exactly two things here: tell HQ it exists (with the
//! address to dispatch to and the Discord bot it logged in as), and keep saying
//! so, along with the guilds that bot is a member of. HQ places sessions from
//! that information alone.

use std::sync::Arc;
use std::time::Duration;

use ae_protocol::AeAdvertisement;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::http_client::{HeaderMap, HeaderValue, HttpClient, HttpClientBuilder};
use jsonrpsee::rpc_params;
use zako3_types::GuildId;

/// The header HQ's RPC surface authenticates on. The engine already carries the
/// admin token for the audio plane, so registration reuses it rather than
/// inventing a second credential.
const ADMIN_TOKEN_HEADER: &str = "x-admin-token";

/// How often the engine refreshes its registration, and how often it re-states
/// the guilds its bot is in.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const GUILD_REPORT_INTERVAL: Duration = Duration::from_secs(60);

/// Registration is a small, infrequent call; giving it its own short budget
/// keeps a hung HQ from stalling startup indefinitely.
const REGISTRATION_TIMEOUT: Duration = Duration::from_secs(10);

pub struct HqRegistryClient {
    client: HttpClient,
}

impl HqRegistryClient {
    pub fn new(url: &str, admin_token: &str) -> anyhow::Result<Arc<Self>> {
        let mut headers = HeaderMap::new();
        headers.insert(ADMIN_TOKEN_HEADER, HeaderValue::from_str(admin_token)?);

        let client = HttpClientBuilder::default()
            .request_timeout(REGISTRATION_TIMEOUT)
            .set_headers(headers)
            .build(url)?;

        Ok(Arc::new(Self { client }))
    }

    pub async fn register(&self, advertisement: AeAdvertisement) -> Result<(), String> {
        self.client
            .request("register_ae", rpc_params![advertisement])
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn heartbeat(&self, advertisement: AeAdvertisement) -> Result<(), String> {
        self.client
            .request("heartbeat_ae", rpc_params![advertisement])
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn report_guilds(
        &self,
        client_id: String,
        guilds: Vec<GuildId>,
    ) -> Result<(), String> {
        self.client
            .request("report_guilds", rpc_params![client_id, guilds])
            .await
            .map_err(|e| e.to_string())
    }
}

/// The guilds this engine's bot is currently a member of, read from its own
/// Discord cache.
fn current_guilds(ctx: &serenity::all::Context) -> Vec<GuildId> {
    ctx.cache
        .guilds()
        .into_iter()
        .map(|g: serenity::model::id::GuildId| GuildId::from(g.get()))
        .collect()
}

/// Reports the guild list to HQ once. Fire-and-forget friendly.
pub async fn report_guilds_once(
    ctx: &serenity::all::Context,
    registry: &HqRegistryClient,
    client_id: &str,
) {
    let guilds = current_guilds(ctx);
    tracing::debug!(guild_count = guilds.len(), "Reporting guild list to HQ");

    if let Err(e) = registry.report_guilds(client_id.to_string(), guilds).await {
        tracing::warn!("guild reporter: report_guilds failed: {e}");
    }
}

/// Periodically reports the guild list to HQ. Runs forever.
pub async fn run_guild_reporter(
    ctx: serenity::all::Context,
    registry: Arc<HqRegistryClient>,
    client_id: String,
) {
    loop {
        report_guilds_once(&ctx, &registry, &client_id).await;
        tokio::time::sleep(GUILD_REPORT_INTERVAL).await;
    }
}

/// Keeps this engine's registration fresh. Runs forever, sleeping first so the
/// startup registration is not immediately repeated.
///
/// HQ expires an engine that stops heartbeating, so this is also how an engine
/// that briefly lost HQ puts itself back into the placement pool.
pub async fn run_ae_heartbeat(registry: Arc<HqRegistryClient>, advertisement: AeAdvertisement) {
    let sink_id = advertisement.sink_id.clone();
    loop {
        tokio::time::sleep(HEARTBEAT_INTERVAL).await;
        match registry.heartbeat(advertisement.clone()).await {
            Ok(()) => tracing::debug!(%sink_id, "ae_heartbeat: registration refreshed"),
            Err(e) => tracing::warn!(%sink_id, "ae_heartbeat: heartbeat failed: {e}"),
        }
    }
}
