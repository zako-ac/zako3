use async_trait::async_trait;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use zako3_types::{
    ZakoError, ZakoResult,
    hq::{DiscordUserId, Tap, User, rpc::HqRpcClient},
};

use crate::repository::HqRepository;

pub struct RpcHqRepository {
    http_client: HttpClient,
}

impl RpcHqRepository {
    pub fn new(url: &str) -> ZakoResult<Self> {
        let http_client = HttpClientBuilder::default()
            .build(url)
            .map_err(|e| ZakoError::Rpc(e.to_string()))?;

        Ok(Self { http_client })
    }
}

#[async_trait]
impl HqRepository for RpcHqRepository {
    async fn authenticate_tap(&self, token: &str) -> Option<Tap> {
        self.http_client
            .authenticate_tap(token.to_string())
            .await
            .inspect_err(|err| {
                tracing::warn!("Failed to authenticate tap: {}", err);
            })
            .ok()?
    }

    async fn get_tap_by_id(&self, tap_id: &str) -> Option<Tap> {
        self.http_client
            .get_tap_internal(tap_id.to_string())
            .await
            .inspect_err(|err| {
                tracing::warn!("Failed to get tap with id {}: {}", tap_id, err);
            })
            .ok()?
    }

    async fn get_user_by_discord_id(&self, discord_id: &DiscordUserId) -> Option<User> {
        self.http_client
            .get_user_by_discord_id(discord_id.0.clone())
            .await
            .inspect_err(|err| {
                tracing::warn!(
                    "Failed to get user with discord id {}: {}",
                    discord_id.0,
                    err
                );
            })
            .ok()?
    }
}
