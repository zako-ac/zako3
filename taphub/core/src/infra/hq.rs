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
    pub fn new(url: &str, admin_token: &str) -> ZakoResult<Self> {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            http::HeaderName::from_static("x-admin-token"),
            http::HeaderValue::from_str(admin_token).map_err(|e| ZakoError::Rpc(e.to_string()))?,
        );

        let http_client = HttpClientBuilder::default()
            .set_headers(headers)
            .build(url)
            .map_err(|e| ZakoError::Rpc(e.to_string()))?;

        Ok(Self { http_client })
    }
}

#[async_trait]
impl HqRepository for RpcHqRepository {
    #[tracing::instrument(skip(self, token), name = "hq.rpc.authenticate_tap")]
    async fn authenticate_tap(&self, token: &str) -> Option<Tap> {
        self.http_client
            .authenticate_tap(token.to_string())
            .await
            .inspect_err(|err| {
                tracing::warn!("Failed to authenticate tap: {}", err);
            })
            .ok()?
    }

    #[tracing::instrument(skip(self), name = "hq.rpc.get_tap", fields(tap_id))]
    async fn get_tap_by_id(&self, tap_id: &str) -> Option<Tap> {
        tracing::Span::current().record("tap_id", tap_id);
        self.http_client
            .get_tap_internal(tap_id.to_string())
            .await
            .inspect_err(|err| {
                tracing::warn!("Failed to get tap with id {}: {}", tap_id, err);
            })
            .ok()?
    }

    #[tracing::instrument(skip(self), name = "hq.rpc.get_user", fields(discord_id = %discord_id.0))]
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
