use crate::hq::{Tap, User};
use jsonrpsee::proc_macros::rpc;

#[rpc(server, client)]
pub trait HqRpc {
    #[method(name = "authenticate_tap")]
    async fn authenticate_tap(&self, token: String) -> jsonrpsee::core::RpcResult<Option<Tap>>;

    #[method(name = "get_tap_internal")]
    async fn get_tap_internal(&self, tap_id: String) -> jsonrpsee::core::RpcResult<Option<Tap>>;

    #[method(name = "get_user_by_discord_id")]
    async fn get_user_by_discord_id(
        &self,
        discord_id: String,
    ) -> jsonrpsee::core::RpcResult<Option<User>>;

    #[method(name = "list_users")]
    async fn list_users(&self) -> jsonrpsee::core::RpcResult<Vec<User>>;

    #[method(name = "get_user")]
    async fn get_user(&self, user_id: String) -> jsonrpsee::core::RpcResult<Option<User>>;

    #[method(name = "update_user_permissions")]
    async fn update_user_permissions(
        &self,
        user_id: String,
        permissions: Vec<String>,
    ) -> jsonrpsee::core::RpcResult<User>;

    #[method(name = "list_taps")]
    async fn list_taps(&self, owner_id: Option<String>) -> jsonrpsee::core::RpcResult<Vec<Tap>>;

    #[method(name = "get_tap")]
    async fn get_tap(&self, tap_id: String) -> jsonrpsee::core::RpcResult<Option<Tap>>;

    #[method(name = "delete_tap")]
    async fn delete_tap(&self, tap_id: String) -> jsonrpsee::core::RpcResult<()>;

    #[method(name = "verify_tap_permission")]
    async fn verify_tap_permission(
        &self,
        tap_id: String,
        discord_user_id: String,
    ) -> jsonrpsee::core::RpcResult<bool>;
}
