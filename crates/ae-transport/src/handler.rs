use std::collections::HashMap;

use async_trait::async_trait;
use tl_protocol::{AudioEngineCommandRequest, AudioEngineCommandResponse};

#[async_trait]
pub trait TlClientHandler: Send + Sync + 'static {
    async fn handle(
        &self,
        req: AudioEngineCommandRequest,
        headers: &HashMap<String, String>,
    ) -> AudioEngineCommandResponse;
}
