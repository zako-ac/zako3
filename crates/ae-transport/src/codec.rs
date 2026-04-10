use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::TlError;

pub(crate) async fn send_frame<T: Serialize>(
    framed: &mut Framed<TcpStream, LengthDelimitedCodec>,
    val: &T,
) -> Result<(), TlError> {
    let bytes = Bytes::from(serde_json::to_vec(val)?);
    framed.send(bytes).await?;
    Ok(())
}

pub(crate) async fn recv_frame<T: for<'de> Deserialize<'de>>(
    framed: &mut Framed<TcpStream, LengthDelimitedCodec>,
) -> Result<T, TlError> {
    let frame = framed
        .next()
        .await
        .ok_or(TlError::ConnectionClosed)?
        .map_err(TlError::Io)?;
    Ok(serde_json::from_slice(&frame)?)
}
