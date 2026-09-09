//! JSON-RPC client for HQ's audio plane.
//!
//! Mirrors `RealTapHubService`'s shape: trace context is injected into the
//! request headers so a play can be followed from the bot through HQ to the
//! tap, and every call keeps its own timeout inside the engine's per-attempt
//! budget.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use jsonrpsee::core::client::ClientT;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use jsonrpsee::rpc_params;
use opentelemetry::global;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use zako3_audio_engine_core::error::{ZakoError, ZakoResult};
use zako3_types::TapHubError;
use zako3_types::hq::audio_dispatch::{AudioDispatch, MetaDispatch, SinkTicket, StreamReport};
use zako3_types::{AudioRequest, CachedAudioRequest};

const ADMIN_TOKEN_HEADER: &str = "x-admin-token";

pub struct HqAudioClient {
    client: HttpClient,
}

impl HqAudioClient {
    pub fn new(url: &str, admin_token: &str, timeout: Duration) -> anyhow::Result<Arc<Self>> {
        let mut headers = jsonrpsee::http_client::HeaderMap::new();
        headers.insert(
            ADMIN_TOKEN_HEADER,
            jsonrpsee::http_client::HeaderValue::from_str(admin_token)?,
        );

        let client = HttpClientBuilder::default()
            .request_timeout(timeout)
            .set_headers(headers)
            .build(url)?;

        Ok(Arc::new(Self { client }))
    }

    pub async fn request_audio(
        &self,
        sink_id: &str,
        ticket: SinkTicket,
        mut request: CachedAudioRequest,
    ) -> ZakoResult<AudioDispatch> {
        inject_trace(&mut request.headers);
        self.call("request_audio", rpc_params![sink_id, ticket, request])
            .await
    }

    pub async fn request_audio_meta(
        &self,
        mut request: AudioRequest,
    ) -> ZakoResult<MetaDispatch> {
        inject_trace(&mut request.headers);
        self.call("request_audio_meta", rpc_params![request]).await
    }

    pub async fn preload_audio(
        &self,
        mut request: CachedAudioRequest,
    ) -> ZakoResult<MetaDispatch> {
        inject_trace(&mut request.headers);
        self.call("preload_audio", rpc_params![request]).await
    }

    pub async fn invalidate_cache(&self, request: CachedAudioRequest) -> ZakoResult<()> {
        self.call("invalidate_cache", rpc_params![request]).await
    }

    /// Report a finished transfer. Best effort: losing it costs visibility, not
    /// correctness, and failing a play because a report did not land would be
    /// strictly worse.
    pub async fn report_stream_outcome(&self, request_id: uuid::Uuid, report: StreamReport) {
        let res: Result<(), _> = self
            .client
            .request("report_stream_outcome", rpc_params![request_id, report])
            .await;
        if let Err(e) = res {
            tracing::warn!(%e, %request_id, "failed to report the stream outcome to HQ");
        }
    }

    /// Unwrap the two layers HQ answers with: transport failure on the outside,
    /// the domain answer on the inside.
    ///
    /// Keeping them apart is what lets a `TapHubError` reach the Discord bot
    /// intact and be turned into a localized message, instead of arriving as an
    /// opaque JSON-RPC string.
    async fn call<T: serde::de::DeserializeOwned>(
        &self,
        method: &'static str,
        params: jsonrpsee::core::params::ArrayParams,
    ) -> ZakoResult<T> {
        let outcome: Result<T, TapHubError> = self
            .client
            .request(method, params)
            .await
            .map_err(|e| ZakoError::TapHub(TapHubError::Internal(format!("{method}: {e}"))))?;
        outcome.map_err(ZakoError::TapHub)
    }
}

fn inject_trace(headers: &mut HashMap<String, String>) {
    let cx = tracing::Span::current().context();
    global::get_text_map_propagator(|p| p.inject_context(&cx, headers));
}
