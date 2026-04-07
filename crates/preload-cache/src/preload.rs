use std::path::{Path, PathBuf};

use bytes::Bytes;
use tokio::{
    fs,
    io::{self, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::mpsc,
};
use tracing::warn;

use crate::types::{NextFrame, PreloadId};

// ---------------------------------------------------------------------------
// AudioPreload — file-backed Opus frame writer
// ---------------------------------------------------------------------------

pub struct AudioPreload {
    dir: PathBuf,
    max_file_bytes: Option<u64>,
}

impl AudioPreload {
    pub fn new(dir: PathBuf, max_file_bytes: Option<u64>) -> Self {
        Self { dir, max_file_bytes }
    }

    pub fn frame_path(&self, id: PreloadId) -> PathBuf {
        self.dir.join(format!("{}.opus", id.0))
    }

    fn lock_path(&self, id: PreloadId) -> PathBuf {
        self.dir.join(format!("{}.lock", id.0))
    }

    /// Spawns a task that drains `stream` and writes frames to disk.
    /// Returns immediately. Any I/O error during writing deletes both files.
    pub fn preload(&self, id: PreloadId, stream: mpsc::Receiver<Bytes>) {
        let frame_path = self.frame_path(id);
        let lock_path = self.lock_path(id);
        tokio::spawn(write_task(frame_path, lock_path, stream, self.max_file_bytes));
    }

    /// Deletes the frame file and lock file for `id` if they exist.
    pub async fn delete_preload(&self, id: PreloadId) -> io::Result<()> {
        remove_if_exists(&self.frame_path(id)).await?;
        remove_if_exists(&self.lock_path(id)).await?;
        Ok(())
    }

    /// Opens a reader for `id`. Returns `None` if the frame file doesn't exist yet.
    pub async fn open_reader(&self, id: PreloadId) -> Option<PreloadReader> {
        let frame_path = self.frame_path(id);
        let lock_path = self.lock_path(id);
        let file = fs::File::open(&frame_path).await.ok()?;
        Some(PreloadReader { file: BufReader::new(file), lock_path })
    }
}

// ---------------------------------------------------------------------------
// Write task
// ---------------------------------------------------------------------------

async fn write_task(
    frame_path: PathBuf,
    lock_path: PathBuf,
    mut stream: mpsc::Receiver<Bytes>,
    max_file_bytes: Option<u64>,
) {
    let result = async {
        let mut file = fs::File::create(&frame_path).await?;
        fs::File::create(&lock_path).await?;

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
        }

        file.flush().await?;
        file.sync_data().await?;
        fs::remove_file(&lock_path).await?;

        io::Result::Ok(())
    }
    .await;

    if let Err(e) = result {
        warn!(?e, "preload write failed, cleaning up");
        let _ = fs::remove_file(&frame_path).await;
        let _ = fs::remove_file(&lock_path).await;
    }
}

// ---------------------------------------------------------------------------
// PreloadReader
// ---------------------------------------------------------------------------

pub struct PreloadReader {
    pub(crate) file: BufReader<fs::File>,
    pub(crate) lock_path: PathBuf,
}

impl PreloadReader {
    pub async fn next_frame(&mut self) -> io::Result<NextFrame> {
        let mut len_buf = [0u8; 4];
        match self.file.read_exact(&mut len_buf).await {
            Ok(_) => {
                let frame_len = u32::from_le_bytes(len_buf) as usize;
                let mut buf = vec![0u8; frame_len];
                self.file.read_exact(&mut buf).await?;
                Ok(NextFrame::Frame(Bytes::from(buf)))
            }
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                if self.lock_path.exists() {
                    Ok(NextFrame::Pending)
                } else {
                    Ok(NextFrame::Done)
                }
            }
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
                cache.store_from_path(item, metadatas, cache_key, &opus_path).await?;
                preload.delete_preload(preload_id).await?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
