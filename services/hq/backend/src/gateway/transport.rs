//! Adapts an axum WebSocket to the driver's [`Transport`].
//!
//! `zakofish4-hub` is written against a stream of binary frames rather than
//! axum's type, which is why the protocol is testable over an in-memory pipe.
//! This is the whole cost of that choice.

use async_trait::async_trait;
use axum::extract::ws::{Message, WebSocket};
use futures_util::StreamExt;
use zakofish4_hub::Transport;

pub struct AxumTransport(WebSocket);

impl AxumTransport {
    pub fn new(socket: WebSocket) -> Self {
        Self(socket)
    }
}

#[async_trait]
impl Transport for AxumTransport {
    type Error = axum::Error;

    async fn recv(&mut self) -> Option<Result<Vec<u8>, Self::Error>> {
        loop {
            match self.0.next().await? {
                Ok(Message::Binary(b)) => return Some(Ok(b)),
                Ok(Message::Close(_)) => return None,
                // The gateway runs its own Ping/Pong inside the protocol,
                // because a WebSocket-level ping does not prove the tap's
                // application loop is alive — only that its TCP stack is.
                Ok(_) => continue,
                Err(e) => return Some(Err(e)),
            }
        }
    }

    async fn send(&mut self, frame: Vec<u8>) -> Result<(), Self::Error> {
        self.0.send(Message::Binary(frame)).await
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        self.0.send(Message::Close(None)).await
    }
}
