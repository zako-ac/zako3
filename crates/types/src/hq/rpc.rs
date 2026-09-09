use crate::hq::audio_dispatch::{AudioDispatch, MetaDispatch, SinkTicket, StreamReport};
use crate::hq::{Tap, User};
use crate::{AudioRequest, CachedAudioRequest, TapHubError};
use jsonrpsee::proc_macros::rpc;
use uuid::Uuid;

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

    // --- audio plane -------------------------------------------------------
    //
    // Called by audio engines. HQ resolves the tap, checks permission and the
    // cache, and dispatches to a tap over the gateway WebSocket; the audio
    // itself never passes through here.

    /// Serve an audio request.
    ///
    /// `ticket` must already be armed by the caller — HQ may reach the tap, and
    /// the tap may start sending, before this call returns.
    #[method(name = "request_audio")]
    async fn request_audio(
        &self,
        ae_id: String,
        ticket: SinkTicket,
        request: CachedAudioRequest,
    ) -> jsonrpsee::core::RpcResult<Result<AudioDispatch, TapHubError>>;

    /// Metadata only. Never touches UDP, so this is the easiest path to migrate.
    #[method(name = "request_audio_meta")]
    async fn request_audio_meta(
        &self,
        request: AudioRequest,
    ) -> jsonrpsee::core::RpcResult<Result<MetaDispatch, TapHubError>>;

    /// Fill the cache without playing anything. No audio engine is involved:
    /// HQ obtains a ticket from the cache worker, which receives the stream.
    #[method(name = "preload_audio")]
    async fn preload_audio(
        &self,
        request: CachedAudioRequest,
    ) -> jsonrpsee::core::RpcResult<Result<MetaDispatch, TapHubError>>;

    /// Drop a cached entry that failed to decode.
    ///
    /// Without this a poisoned entry keeps being served and keeps failing, for
    /// as long as it survives eviction.
    #[method(name = "invalidate_cache")]
    async fn invalidate_cache(
        &self,
        request: CachedAudioRequest,
    ) -> jsonrpsee::core::RpcResult<Result<(), TapHubError>>;

    /// Report how a transfer ended.
    ///
    /// The audio engine sees the stream and HQ does not, so this is the only
    /// way HQ learns the difference between "dispatched" and "delivered".
    #[method(name = "report_stream_outcome")]
    async fn report_stream_outcome(
        &self,
        request_id: Uuid,
        report: StreamReport,
    ) -> jsonrpsee::core::RpcResult<()>;
}
