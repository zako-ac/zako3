use tokio::io::AsyncRead;

use crate::{error::ZakoResult, types::StreamCacheKey};

pub trait StreamCacheService: Send + Sync + 'static {
    async fn write(
        &self,
        key: &StreamCacheKey,
        stream: impl AsyncRead + Send + Unpin + 'static,
    ) -> ZakoResult<impl AsyncRead + Send + Unpin + 'static>;
    async fn read(
        &self,
        key: &StreamCacheKey,
    ) -> ZakoResult<Option<impl AsyncRead + Send + Unpin + 'static>>;
    async fn has(&self, key: &StreamCacheKey) -> ZakoResult<bool>;
    async fn delete(&self, key: &StreamCacheKey) -> ZakoResult<()>;
}
