use async_trait::async_trait;
use tl_protocol::{AudioEngineCommandRequest, AudioEngineCommandResponse};

use crate::{SessionRoute, TlError};

#[async_trait]
pub trait AeDispatcher: Send + Sync {
    async fn send(
        &self,
        route: SessionRoute,
        request: AudioEngineCommandRequest,
    ) -> Result<AudioEngineCommandResponse, TlError>;
}
