use crate::hq::Tap;
use jsonrpsee::proc_macros::rpc;

#[rpc(server, client)]
pub trait HqRpc {
    #[method(name = "authenticate_tap")]
    async fn authenticate_tap(&self, token: String) -> jsonrpsee::core::RpcResult<Option<Tap>>;

    #[method(name = "get_tap_internal")]
    async fn get_tap_internal(&self, tap_id: String) -> jsonrpsee::core::RpcResult<Option<Tap>>;
}
