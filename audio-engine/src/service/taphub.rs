use mockall::automock;
use tokio::io::AsyncRead;

use crate::{error::ZakoResult, types::AudioRequest};

#[automock]
pub trait TapHubService: Send + Sync + 'static {
    async fn request_audio(
        &self,
        request: AudioRequest,
    ) -> ZakoResult<Box<dyn AsyncRead + Send + Unpin>>;
}
