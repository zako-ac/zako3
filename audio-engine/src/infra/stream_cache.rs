use std::{path::PathBuf, sync::Arc};

use tokio::io::AsyncRead;
use tokio_util::io::InspectReader;

use crate::{error::ZakoResult, service::StreamCacheService, types::StreamCacheKey};

pub struct FileStreamCache {
    pub base_path: PathBuf,
}

impl StreamCacheService for FileStreamCache {
    async fn write(
        &self,
        key: &StreamCacheKey,
        stream: impl AsyncRead + Send + Unpin + 'static,
    ) -> ZakoResult<impl AsyncRead + Send + Unpin + 'static> {
        let file_path = self.base_path.join(String::from(key.clone()));

        let mut file = Arc::new(std::fs::File::create(file_path)?);

        let teed_reader = InspectReader::new(stream, move |chunk: &[u8]| {
            use std::io::Write;
            let _ = file.write_all(chunk); // TODO Profiler
        });

        Ok(teed_reader)
    }

    async fn read(
        &self,
        key: &StreamCacheKey,
    ) -> ZakoResult<Option<impl AsyncRead + Send + Unpin + 'static>> {
        let file_path = self.base_path.join(String::from(key.clone()));

        if !file_path.exists() {
            return Ok(None);
        }

        let file = tokio::fs::File::open(file_path).await?;

        Ok(Some(file))
    }

    async fn has(&self, key: &StreamCacheKey) -> ZakoResult<bool> {
        let file_path = self.base_path.join(String::from(key.clone()));

        Ok(file_path.exists())
    }

    async fn delete(&self, key: &StreamCacheKey) -> ZakoResult<()> {
        let file_path = self.base_path.join(String::from(key.clone()));

        if file_path.exists() {
            tokio::fs::remove_file(file_path).await?;
        }

        Ok(())
    }
}
