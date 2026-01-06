use tokio::io::AsyncRead;

use crate::{error::ZakoResult, types::AudioRequest};

pub trait TapHubService: Send + Sync + 'static {
    async fn request_audio(
        &self,
        request: AudioRequest,
    ) -> ZakoResult<impl AsyncRead + Send + Unpin + 'static>;
}
