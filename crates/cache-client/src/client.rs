use std::path::Path;

use async_trait::async_trait;
use bytes::{BufMut, Bytes, BytesMut};
use futures_util::StreamExt;
use reqwest::{StatusCode, header};
use tokio::io;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::io::StreamReader;
use zako3_preload_cache::{AudioCache, CacheEntry, PreloadReader};
use zako3_types::{
    AudioCachePolicy, AudioMetadata,
    cache::{AudioCacheItem, AudioCacheItemKey},
    hq::TapId,
};

use crate::dto::{
    CacheEntryDto, CreatePreloadReq, EntryQuery, PreloadCreatedResp, StoreMetadataReq,
};

const ADMIN_TOKEN_HEADER: &str = "x-admin-token";

/// Implements [`AudioCache`] over HTTP against the zako3 cache server.
pub struct RemoteAudioCache {
    http: reqwest::Client,
    base_url: String,
    admin_token: Option<String>,
}

impl RemoteAudioCache {
    pub fn new(base_url: impl Into<String>, admin_token: Option<String>) -> reqwest::Result<Self> {
        // No total-request timeout: audio uploads stream as long as the upstream
        // Tap keeps sending. taphub enforces its own per-request timeout.
        let http = reqwest::Client::builder().build()?;
        Ok(Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            admin_token,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut req = self.http.request(method, self.url(path));
        if let Some(token) = &self.admin_token {
            req = req.header(ADMIN_TOKEN_HEADER, token);
        }
        req
    }
}

fn io_other<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::other(e.to_string())
}

/// Wrap each `Bytes` frame from `rx` into a `<u32 LE len><bytes>` chunk, identical
/// to what `FileAudioCache` writes to disk. This is what the server expects on
/// `POST /preload/{id}/frames`.
fn framed_body(rx: mpsc::Receiver<Bytes>) -> reqwest::Body {
    let stream = ReceiverStream::new(rx).map(|frame| {
        let mut out = BytesMut::with_capacity(4 + frame.len());
        out.put_u32_le(frame.len() as u32);
        out.extend_from_slice(&frame);
        Ok::<Bytes, std::io::Error>(out.freeze())
    });
    reqwest::Body::wrap_stream(stream)
}

#[async_trait]
impl AudioCache for RemoteAudioCache {
    async fn store(
        &self,
        item: AudioCacheItem,
        metadatas: Vec<AudioMetadata>,
        cache_key: AudioCachePolicy,
        stream: mpsc::Receiver<Bytes>,
        done: oneshot::Receiver<()>,
    ) -> io::Result<()> {
        // 1. Create a preload session bound to the cache target.
        let create = CreatePreloadReq {
            item,
            metadatas,
            cache_key,
        };
        let resp = self
            .request(reqwest::Method::POST, "/preload")
            .json(&create)
            .send()
            .await
            .map_err(io_other)?;
        if !resp.status().is_success() {
            return Err(io_other(format!(
                "POST /preload failed: {}",
                resp.status()
            )));
        }
        let created: PreloadCreatedResp = resp.json().await.map_err(io_other)?;
        let preload_id = created.preload_id;

        // 2. Upload frames. The body finishes when the mpsc sender drops.
        let frames_path = format!("/preload/{preload_id}/frames");
        let upload = self
            .request(reqwest::Method::POST, &frames_path)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(framed_body(stream))
            .send();

        // 3. Decide commit vs abort based on `done`. The upload future blocks
        //    until the body is fully consumed. Run both concurrently so we know
        //    by the time we reach commit whether the producer signaled success.
        let (upload_res, done_res) = tokio::join!(upload, done);
        let upload_ok = upload_res
            .as_ref()
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        let succeeded = done_res.is_ok() && upload_ok;

        let endpoint = if succeeded { "commit" } else { "abort" };
        let finalize = self
            .request(
                reqwest::Method::POST,
                &format!("/preload/{preload_id}/{endpoint}"),
            )
            .send()
            .await
            .map_err(io_other)?;
        if !finalize.status().is_success() {
            return Err(io_other(format!(
                "POST /preload/{preload_id}/{endpoint} failed: {}",
                finalize.status()
            )));
        }

        if !succeeded {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "audio stream did not complete cleanly",
            ));
        }
        Ok(())
    }

    async fn store_from_path(
        &self,
        _item: AudioCacheItem,
        _metadatas: Vec<AudioMetadata>,
        _cache_key: AudioCachePolicy,
        _opus_path: &Path,
    ) -> io::Result<()> {
        // `store_from_path` was used by the local preload finalizer to move a
        // completed `.opus` file into the cache. With remote cache, preload runs
        // entirely server-side and there is no local path to hand off.
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "store_from_path is not supported on RemoteAudioCache; use store()",
        ))
    }

    async fn open_reader(&self, tap_id: &TapId, key: &AudioCacheItemKey) -> Option<PreloadReader> {
        let q = EntryQuery::new(tap_id, key);
        let resp = self
            .request(reqwest::Method::GET, "/stream")
            .query(&q)
            .send()
            .await
            .inspect_err(|e| tracing::warn!(%e, "GET /stream failed"))
            .ok()?;
        if resp.status() == StatusCode::NOT_FOUND {
            return None;
        }
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "GET /stream returned error");
            return None;
        }
        let body = resp.bytes_stream();
        let reader = StreamReader::new(body.map(|r| r.map_err(io::Error::other)));
        Some(PreloadReader::from_reader(reader, None))
    }

    async fn get_entry(&self, tap_id: &TapId, key: &AudioCacheItemKey) -> Option<CacheEntry> {
        let q = EntryQuery::new(tap_id, key);
        let resp = self
            .request(reqwest::Method::GET, "/entry")
            .query(&q)
            .send()
            .await
            .inspect_err(|e| tracing::warn!(%e, "GET /entry failed"))
            .ok()?;
        if resp.status() == StatusCode::NOT_FOUND {
            return None;
        }
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "GET /entry returned error");
            return None;
        }
        let dto: CacheEntryDto = resp
            .json()
            .await
            .inspect_err(|e| tracing::warn!(%e, "GET /entry deserialize failed"))
            .ok()?;
        Some(dto.into())
    }

    async fn store_metadata(
        &self,
        item: AudioCacheItem,
        metadatas: Vec<AudioMetadata>,
        cache_key: AudioCachePolicy,
    ) -> io::Result<()> {
        let body = StoreMetadataReq {
            item,
            metadatas,
            cache_key,
        };
        let resp = self
            .request(reqwest::Method::POST, "/metadata")
            .json(&body)
            .send()
            .await
            .map_err(io_other)?;
        if !resp.status().is_success() {
            return Err(io_other(format!(
                "POST /metadata failed: {}",
                resp.status()
            )));
        }
        Ok(())
    }

    async fn delete(&self, tap_id: &TapId, key: &AudioCacheItemKey) -> io::Result<()> {
        let q = EntryQuery::new(tap_id, key);
        let resp = self
            .request(reqwest::Method::DELETE, "/entry")
            .query(&q)
            .send()
            .await
            .map_err(io_other)?;
        if !resp.status().is_success() && resp.status() != StatusCode::NOT_FOUND {
            return Err(io_other(format!(
                "DELETE /entry failed: {}",
                resp.status()
            )));
        }
        Ok(())
    }
}
