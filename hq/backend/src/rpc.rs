use hq_core::service::api_key::ApiKeyService;
use hq_core::service::tap::TapService;
use hq_types::hq::rpc::HqRpcServer;
use hq_types::hq::Tap;
use hq_types::ZakoResult;
use jsonrpsee::core::async_trait;
use jsonrpsee::core::RpcResult;
use jsonrpsee::types::ErrorObjectOwned;
use uuid::Uuid;

pub struct HqRpcImpl {
    api_key_service: ApiKeyService,
    tap_service: TapService,
}

impl HqRpcImpl {
    pub fn new(api_key_service: ApiKeyService, tap_service: TapService) -> Self {
        Self {
            api_key_service,
            tap_service,
        }
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
        let uuid = match Uuid::parse_str(&tap_id) {
            Ok(u) => u,
            Err(_) => return Ok(None),
        };
        let res = self.tap_service.get_tap_internal(uuid).await;
        match res {
            Ok(tap) => Ok(tap),
            Err(e) => Err(ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>)),
        }
    }
}

pub async fn start_rpc_server(
    api_key_service: ApiKeyService,
    tap_service: TapService,
    address: &str,
) -> ZakoResult<()> {
    let server = jsonrpsee::server::Server::builder().build(address).await?;

    let handle = server.start(HqRpcImpl::new(api_key_service, tap_service).into_rpc());
    tracing::info!("RPC server listening on {}", address);

    handle.stopped().await;

    Ok(())
}
