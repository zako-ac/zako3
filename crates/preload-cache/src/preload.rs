use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bytes::Bytes;
use tokio::io::{AsyncRead, BufWriter};
use tokio::{
    fs,
    io::{self, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::{Notify, mpsc},
    time::Duration,
};
use tracing::warn;

use crate::types::{NextFrame, PreloadId};

pub struct WriteSignal {
    pub notify: Notify,
    pub done: AtomicBool,
}

impl WriteSignal {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            notify: Notify::new(),
            done: AtomicBool::new(false),
        })
    }

    fn finish(&self) {
        self.done.store(true, Ordering::Release);
        self.notify.notify_one();
    }
}

pub struct AudioPreload {
    dir: PathBuf,
    max_file_bytes: Option<u64>,
}

impl AudioPreload {
    pub fn new(dir: PathBuf, max_file_bytes: Option<u64>) -> Self {
        Self {
            dir,
            max_file_bytes,
        }
    }

    pub fn frame_path(&self, id: PreloadId) -> PathBuf {
        self.dir.join(format!("{}.opus", id.0))
    }

    fn lock_path(&self, id: PreloadId) -> PathBuf {
        self.dir.join(format!("{}.lock", id.0))
    }

    /// Spawns a task that drains `stream` and writes frames to disk.
    /// Returns a `WriteSignal` that fires when the file is ready to read and after each frame.
    pub fn preload(&self, id: PreloadId, stream: mpsc::Receiver<Bytes>) -> Arc<WriteSignal> {
        let signal = WriteSignal::new();
        let frame_path = self.frame_path(id);
        let lock_path = self.lock_path(id);

        tracing::info!(
            "started preload write task for id {}. frame_path: {:?}, lock_path: {:?}",
            id.0,
            frame_path,
            lock_path
        );

        tokio::spawn(write_task(
            frame_path,
            lock_path,
            stream,
            self.max_file_bytes,
            Arc::clone(&signal),
        ));
        signal
    }

    /// Deletes the frame file and lock file for `id` if they exist.
    pub async fn delete_preload(&self, id: PreloadId) -> io::Result<()> {
        remove_if_exists(&self.frame_path(id)).await?;
        remove_if_exists(&self.lock_path(id)).await?;
        Ok(())
    }

    /// Opens a reader for `id` with no signal (returns `Done` on EOF).
    /// Returns `None` if the frame file doesn't exist yet.
    pub async fn open_reader(&self, id: PreloadId) -> Option<PreloadReader> {
        let file = fs::File::open(&self.frame_path(id)).await.ok()?;
        Some(PreloadReader::from_file(file, None))
    }

    /// Opens a reader for `id` with a `WriteSignal` for event-driven frame waiting.
    /// Returns `None` if the frame file doesn't exist yet.
    pub async fn open_reader_with_signal(
        &self,
        id: PreloadId,
        signal: Arc<WriteSignal>,
    ) -> Option<PreloadReader> {
        let file = fs::File::open(&self.frame_path(id)).await.ok()?;
        Some(PreloadReader::from_file(file, Some(signal)))
    }
}

async fn write_task(
    frame_path: PathBuf,
    lock_path: PathBuf,
    mut stream: mpsc::Receiver<Bytes>,
    max_file_bytes: Option<u64>,
    signal: Arc<WriteSignal>,
) {
    let result = async {
        let mut file = BufWriter::new(fs::File::create(&frame_path).await?);
        fs::File::create(&lock_path).await?;

        // File is created — wake the finalization task so it can open a reader.
        signal.notify.notify_one();

        let mut total_bytes: u64 = 0;
        while let Some(frame) = stream.recv().await {
            total_bytes += 4 + frame.len() as u64;
            if let Some(max) = max_file_bytes
                && total_bytes > max
            {
                warn!("preload exceeded max_file_bytes ({max}), dropping");
                drop(stream);
                return Err(io::Error::new(
                    io::ErrorKind::FileTooLarge,
                    "preload exceeded max_file_bytes",
                ));
            }
            let len = frame.len() as u32;
            file.write_all(&len.to_le_bytes()).await?;
            file.write_all(&frame).await?;
            file.flush().await?;

            // Wake reader for each new frame.
            signal.notify.notify_one();

            tracing::debug!(
                "wrote frame to preload. frame_len: {}, total_bytes: {}, frame_path: {:?}, stream_len: {}",
                len,
                total_bytes,
                frame_path,
                stream.len(),
            );
        }

        file.flush().await?;
        fs::remove_file(&lock_path).await?;

        tracing::info!(
            "preload write task completed successfully. frame_path: {:?}, lock_path: {:?}",
            frame_path,
            lock_path
        );

        io::Result::Ok(())
    }
    .await;

    if let Err(e) = result {
        warn!(?e, "preload write failed, cleaning up");
        let _ = fs::remove_file(&frame_path).await;
        let _ = fs::remove_file(&lock_path).await;
    }

    // Always signal done so readers don't wait forever.
    signal.finish();
}

pub struct PreloadReader {
    pub(crate) inner: Box<dyn AsyncRead + Send + Unpin>,
    pub(crate) signal: Option<Arc<WriteSignal>>,
}

impl PreloadReader {
    /// Build a `PreloadReader` from a tokio file (wrapped in a `BufReader` for performance).
    pub fn from_file(file: fs::File, signal: Option<Arc<WriteSignal>>) -> Self {
        Self {
            inner: Box::new(BufReader::new(file)),
            signal,
        }
    }

    /// Build a `PreloadReader` from any `AsyncRead` source (e.g. an HTTP response body).
    /// The caller is expected to have already applied any buffering it needs.
    pub fn from_reader<R: AsyncRead + Send + Unpin + 'static>(
        reader: R,
        signal: Option<Arc<WriteSignal>>,
    ) -> Self {
        Self {
            inner: Box::new(reader),
            signal,
        }
    }

    pub async fn next_frame(&mut self) -> io::Result<NextFrame> {
        let mut len_buf = [0u8; 4];
        match self.inner.read_exact(&mut len_buf).await {
            Ok(_) => {
                let frame_len = u32::from_le_bytes(len_buf) as usize;
                let mut buf = vec![0u8; frame_len];
                self.inner.read_exact(&mut buf).await?;
                Ok(NextFrame::Frame(Bytes::from(buf)))
            }
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => match &self.signal {
                None => Ok(NextFrame::Done),
                Some(sig) => {
                    if sig.done.load(Ordering::Acquire) {
                        Ok(NextFrame::Done)
                    } else {
                        tokio::time::timeout(Duration::from_millis(500), sig.notify.notified())
                            .await
                            .ok();
                        Ok(NextFrame::Pending)
                    }
                }
            },
            Err(e) => Err(e),
        }
    }

    /// Call after receiving `NextFrame::Done` to execute the end action.
    /// Consumes the reader to prevent double-finalization.
    pub async fn finalize(
        self,
        preload_id: PreloadId,
        preload: &AudioPreload,
        action: crate::cache::PreloadReadEndAction,
    ) -> io::Result<()> {
        match action {
            crate::cache::PreloadReadEndAction::Delete => {
                preload.delete_preload(preload_id).await?;
            }
            crate::cache::PreloadReadEndAction::MoveToCache {
                item,
                metadatas,
                cache_key,
                cache,
            } => {
                let opus_path = preload.frame_path(preload_id);
                cache
                    .store_from_path(item, metadatas, cache_key, &opus_path)
                    .await?;
                preload.delete_preload(preload_id).await?;
            }
        }
        Ok(())
    }
}

async fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
