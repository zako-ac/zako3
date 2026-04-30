use hq_core::service::api_key::ApiKeyService;
use hq_core::service::auth::AuthService;
use hq_core::service::tap::TapService;
use hq_types::hq::rpc::HqRpcServer;
use hq_types::hq::{Tap, TapId, User, UserId};
use hq_types::ZakoResult;
use jsonrpsee::core::{async_trait, RpcResult};
use jsonrpsee::types::ErrorObjectOwned;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::Service;

pub struct HqRpcImpl {
    api_key_service: ApiKeyService,
    tap_service: TapService,
    auth_service: AuthService,
}

impl HqRpcImpl {
    pub fn new(
        api_key_service: ApiKeyService,
        tap_service: TapService,
        auth_service: AuthService,
    ) -> Self {
        Self {
            api_key_service,
            tap_service,
            auth_service,
        }
    }
}

#[derive(Clone)]
pub struct AuthLayer {
    admin_token: Arc<String>,
}

impl AuthLayer {
    pub fn new(admin_token: String) -> Self {
        Self {
            admin_token: Arc::new(admin_token),
        }
    }
}

impl<S> tower::Layer<S> for AuthLayer {
    type Service = AuthMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthMiddleware {
            inner,
            admin_token: self.admin_token.clone(),
        }
    }
}

#[derive(Clone)]
pub struct AuthMiddleware<S> {
    inner: S,
    admin_token: Arc<String>,
}

impl<S, B> Service<http::Request<B>> for AuthMiddleware<S>
where
    S: Service<http::Request<B>, Response = http::Response<jsonrpsee::server::HttpBody>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<B>) -> Self::Future {
        let auth_header = req
            .headers()
            .get("x-admin-token")
            .and_then(|h| h.to_str().ok());

        if let Some(token) = auth_header {
            if token == *self.admin_token {
                let mut inner = self.inner.clone();
                return Box::pin(async move { inner.call(req).await });
            }
        }

        Box::pin(async move {
            let response = http::Response::builder()
                .status(http::StatusCode::UNAUTHORIZED)
                .body(jsonrpsee::server::HttpBody::from("Unauthorized"))
                .unwrap();
            Ok(response)
        })
    }
}

#[async_trait]
impl HqRpcServer for HqRpcImpl {
    async fn authenticate_tap(&self, token: String) -> RpcResult<Option<Tap>> {
        let res = self.api_key_service.authenticate_tap(&token).await;
        match res {
            Ok(tap) => Ok(tap),
            Err(e) => Err(ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>)),
        }
    }

    async fn get_tap_internal(&self, tap_id: String) -> RpcResult<Option<Tap>> {
        let id = match TapId::from_str(&tap_id) {
            Ok(u) => u,
            Err(_) => return Ok(None),
        };
        let res = self.tap_service.get_tap(id).await;
        match res {
            Ok(tap) => Ok(tap),
            Err(e) => Err(ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>)),
        }
    }

    async fn get_user_by_discord_id(&self, discord_id: String) -> RpcResult<Option<User>> {
        let res = self.tap_service.get_user_by_discord_id(&discord_id).await;
        match res {
            Ok(user) => Ok(user),
            Err(e) => Err(ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>)),
        }
    }

    async fn list_users(&self) -> RpcResult<Vec<User>> {
        let res = self.auth_service.list_all_users(1, 1000).await;
        match res {
            Ok((users, _total)) => Ok(users),
            Err(e) => Err(ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>)),
        }
    }

    async fn get_user(&self, user_id: String) -> RpcResult<Option<User>> {
        let id = match UserId::from_str(&user_id) {
            Ok(u) => u,
            Err(_) => return Ok(None),
        };
        let res = self.auth_service.get_user_internal(id).await;
        match res {
            Ok(user) => Ok(user),
            Err(e) => Err(ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>)),
        }
    }

    async fn update_user_permissions(
        &self,
        user_id: String,
        permissions: Vec<String>,
    ) -> RpcResult<User> {
        let id = UserId::from_str(&user_id)
            .map_err(|e| ErrorObjectOwned::owned(-32602, e.to_string(), None::<()>))?;
        let res = self
            .auth_service
            .update_user_permissions(id, permissions)
            .await;
        match res {
            Ok(user) => Ok(user),
            Err(e) => Err(ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>)),
        }
    }

    async fn list_taps(&self, owner_id: Option<String>) -> RpcResult<Vec<Tap>> {
        let res = if let Some(oid_str) = owner_id {
            let oid = UserId::from_str(&oid_str)
                .map_err(|e| ErrorObjectOwned::owned(-32602, e.to_string(), None::<()>))?;
            self.tap_service.list_taps_by_owner(oid).await
        } else {
            self.tap_service.list_all_taps().await
        };

        match res {
            Ok(taps) => Ok(taps),
            Err(e) => Err(ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>)),
        }
    }

    async fn get_tap(&self, tap_id: String) -> RpcResult<Option<Tap>> {
        let id = match TapId::from_str(&tap_id) {
            Ok(u) => u,
            Err(_) => return Ok(None),
        };
        let res = self.tap_service.get_tap(id).await;
        match res {
            Ok(tap) => Ok(tap),
            Err(e) => Err(ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>)),
        }
    }

    async fn delete_tap(&self, tap_id: String) -> RpcResult<()> {
        let id = TapId::from_str(&tap_id)
            .map_err(|e| ErrorObjectOwned::owned(-32602, e.to_string(), None::<()>))?;
        let res = self.tap_service.delete_tap_internal(id).await;
        match res {
            Ok(_) => Ok(()),
            Err(e) => Err(ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>)),
        }
    }

    async fn verify_tap_permission(
        &self,
        tap_id: String,
        discord_user_id: String,
    ) -> RpcResult<bool> {
        let id = TapId::from_str(&tap_id)
            .map_err(|e| ErrorObjectOwned::owned(-32602, e.to_string(), None::<()>))?;
        let tap = match self.tap_service.get_tap(id).await {
            Ok(Some(t)) => t,
            Ok(None) => return Ok(false),
            Err(e) => return Err(ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>)),
        };
        let user_id = match self.tap_service.get_user_by_discord_id(&discord_user_id).await {
            Ok(Some(u)) => UserId::from_str(&u.id.0).ok(),
            _ => None,
        };
        Ok(self.tap_service.check_access(&tap, user_id).await)
    }
}

pub async fn start_rpc_server(
    api_key_service: ApiKeyService,
    tap_service: TapService,
    auth_service: AuthService,
    address: &str,
    admin_token: String,
) -> ZakoResult<()> {
    let middleware = tower::ServiceBuilder::new().layer(AuthLayer::new(admin_token));

    let server = jsonrpsee::server::Server::builder()
        .set_http_middleware(middleware)
        .build(address)
        .await?;

    let handle =
        server.start(HqRpcImpl::new(api_key_service, tap_service, auth_service).into_rpc());
    tracing::info!("RPC server listening on {}", address);

    handle.stopped().await;

    Ok(())
}
