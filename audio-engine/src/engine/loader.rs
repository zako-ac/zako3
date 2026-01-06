use tokio::io::AsyncRead;

use crate::{
    error::ZakoResult,
    service::{StreamCacheService, TapHubService},
    types::{AudioRequest, StreamCacheKey},
};

pub struct Loader<THS, SCS>
where
    THS: TapHubService,
    SCS: StreamCacheService,
{
    taphub_service: THS,
    stream_cache_service: SCS,
}

impl<THS, SCS> Loader<THS, SCS>
where
    THS: TapHubService,
    SCS: StreamCacheService,
{
    async fn resolve_stream(
        &self,
        request: AudioRequest,
    ) -> ZakoResult<Box<dyn AsyncRead + Send + Unpin + 'static>> {
        let cache_key = make_cache_key(&request);

        if let Some(cached_stream) = self.stream_cache_service.read(&cache_key).await? {
            return Ok(Box::new(cached_stream));
        }

        let original_stream = self.taphub_service.request_audio(request).await?;

        let cached_stream = self
            .stream_cache_service
            .write(&cache_key, original_stream)
            .await?;

        Ok(Box::new(cached_stream))
    }
}

fn make_cache_key(request: &AudioRequest) -> StreamCacheKey {
    format!(
        "{}_{}",
        String::from(request.tap_name.clone()),
        String::from(request.request.clone())
    )
    .into()
}
