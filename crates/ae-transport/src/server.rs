use std::collections::HashMap;
use std::net::ToSocketAddrs;

use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tl_protocol::{AudioEngineCommandRequest, AudioEngineCommandResponse};

use crate::codec::{recv_frame, send_frame};
use crate::{inject_span, DiscordToken, TlClientHandshake, TlError, TlServerHandshake, WireRequest, WireResponse};

pub struct TlServer {
    listener: TcpListener,
}

impl TlServer {
    pub async fn bind(addr: impl ToSocketAddrs) -> Result<Self, TlError> {
        let addr = addr
            .to_socket_addrs()
            .map_err(TlError::Io)?
            .next()
            .ok_or_else(|| TlError::Handshake("no address resolved".into()))?;
        let listener = TcpListener::bind(addr).await?;
        tracing::info!(local_addr = %listener.local_addr()?, "ae-transport: listening");
        Ok(Self { listener })
    }

    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }

    /// Accepts the next incoming TCP connection and performs the handshake.
    /// The caller provides the `DiscordToken` to assign and any server headers.
    pub async fn accept(
        &mut self,
        token: DiscordToken,
        server_headers: HashMap<String, String>,
    ) -> Result<TlConnectedClient, TlError> {
        let (stream, peer_addr) = self.listener.accept().await?;
        tracing::debug!(%peer_addr, "ae-transport: accepted connection");

        let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

        let client_hs: TlClientHandshake = recv_frame(&mut framed).await?;

        send_frame(
            &mut framed,
            &TlServerHandshake {
                token: token.clone(),
                headers: server_headers,
            },
        )
        .await?;

        tracing::debug!(token = %token.0, %peer_addr, "ae-transport: handshake complete");

        Ok(TlConnectedClient {
            token,
            client_headers: client_hs.headers,
            framed,
            next_id: 0,
        })
    }
}

pub struct TlConnectedClient {
    token: DiscordToken,
    client_headers: HashMap<String, String>,
    framed: Framed<TcpStream, LengthDelimitedCodec>,
    next_id: u64,
}

impl TlConnectedClient {
    pub fn token(&self) -> &DiscordToken {
        &self.token
    }

    pub fn client_headers(&self) -> &HashMap<String, String> {
        &self.client_headers
    }

    /// Sends a request to the client (injecting the current tracing span),
    /// then waits for the correlated response.
    pub async fn request(
        &mut self,
        req: AudioEngineCommandRequest,
    ) -> Result<AudioEngineCommandResponse, TlError> {
        let id = self.next_id;
        self.next_id += 1;

        let mut headers = HashMap::new();
        inject_span(&mut headers);

        send_frame(&mut self.framed, &WireRequest { id, headers, payload: req }).await?;

        let wire_resp: WireResponse = recv_frame(&mut self.framed).await?;

        if wire_resp.id != id {
            return Err(TlError::ResponseIdMismatch {
                expected: id,
                got: wire_resp.id,
            });
        }

        Ok(wire_resp.payload)
    }
}
