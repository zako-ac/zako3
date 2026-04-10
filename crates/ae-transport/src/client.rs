use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::Instrument;

use crate::codec::{recv_frame, send_frame};
use crate::handler::TlClientHandler;
use crate::{
    child_span, inject_span, DiscordToken, TlClientHandshake, TlError, TlServerHandshake,
    WireRequest, WireResponse,
};

pub struct TlClient {
    addr: String,
    client_headers: HashMap<String, String>,
    handler: Arc<dyn TlClientHandler>,
}

impl TlClient {
    pub fn new(
        addr: impl Into<String>,
        client_headers: HashMap<String, String>,
        handler: impl TlClientHandler,
    ) -> Self {
        Self {
            addr: addr.into(),
            client_headers,
            handler: Arc::new(handler),
        }
    }

    /// One-shot connect + handshake. Returns the assigned token, server headers,
    /// and a `ConnectedClient` ready to serve requests.
    pub async fn connect(
        addr: impl tokio::net::ToSocketAddrs,
        client_headers: HashMap<String, String>,
    ) -> Result<(DiscordToken, HashMap<String, String>, ConnectedClient), TlError> {
        let stream = TcpStream::connect(addr).await?;
        let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

        let mut headers = client_headers;
        inject_span(&mut headers);
        send_frame(&mut framed, &TlClientHandshake { headers }).await?;

        let server_hs: TlServerHandshake = recv_frame(&mut framed).await?;
        let token = server_hs.token.clone();
        let server_headers = server_hs.headers.clone();

        Ok((token, server_headers, ConnectedClient { framed }))
    }

    /// Connects, handshakes, then serves requests forever, reconnecting on failure
    /// with exponential backoff via `backon`.
    pub async fn run(self) -> Result<(), TlError> {
        let slf = Arc::new(self);

        (|| {
            let s = slf.clone();
            async move { s.connect_and_serve().await }
        })
        .retry(
            ExponentialBuilder::default()
                .with_min_delay(Duration::from_millis(500))
                .with_max_delay(Duration::from_secs(30))
                .with_max_times(usize::MAX),
        )
        .notify(|err: &TlError, dur| {
            tracing::warn!("ae-transport: {err}, reconnecting in {dur:?}");
        })
        .await
    }

    async fn connect_and_serve(self: &Arc<Self>) -> Result<(), TlError> {
        let stream = TcpStream::connect(self.addr.as_str()).await?;
        let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

        let mut headers = self.client_headers.clone();
        inject_span(&mut headers);
        send_frame(&mut framed, &TlClientHandshake { headers }).await?;

        let server_hs: TlServerHandshake = recv_frame(&mut framed).await?;
        tracing::debug!(token = %server_hs.token.0, "ae-transport: handshake complete");

        loop {
            let wire_req: WireRequest = recv_frame(&mut framed).await?;

            let span = child_span(&wire_req.headers);
            let response = self
                .handler
                .handle(wire_req.payload, &wire_req.headers)
                .instrument(span)
                .await;

            send_frame(
                &mut framed,
                &WireResponse {
                    id: wire_req.id,
                    payload: response,
                },
            )
            .await?;
        }
    }
}

/// A connected client after handshake, ready to serve requests from the server.
pub struct ConnectedClient {
    framed: Framed<TcpStream, LengthDelimitedCodec>,
}

impl ConnectedClient {
    pub async fn serve(mut self, handler: impl TlClientHandler) -> Result<(), TlError> {
        loop {
            let wire_req: WireRequest = recv_frame(&mut self.framed).await?;

            let span = child_span(&wire_req.headers);
            let response = handler
                .handle(wire_req.payload, &wire_req.headers)
                .instrument(span)
                .await;

            send_frame(
                &mut self.framed,
                &WireResponse {
                    id: wire_req.id,
                    payload: response,
                },
            )
            .await?;
        }
    }
}
