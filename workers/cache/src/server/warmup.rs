use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use tokio::sync::watch;

/// Progress of the background index warmup.
///
/// The cache serves requests while this is still running — an un-scanned key
/// simply reads as a miss — so this gates nothing on the request path. It exists
/// so `/readyz` can report progress and so the GC loop can hold off until the
/// index is whole.
pub struct WarmupState {
    ready: AtomicBool,
    scanned: AtomicUsize,
    total: AtomicUsize,
    tx: watch::Sender<bool>,
    rx: watch::Receiver<bool>,
}

impl Default for WarmupState {
    fn default() -> Self {
        Self::new()
    }
}

impl WarmupState {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self {
            ready: AtomicBool::new(false),
            scanned: AtomicUsize::new(0),
            total: AtomicUsize::new(0),
            tx,
            rx,
        }
    }

    pub fn record_progress(&self, scanned: usize, total: usize) {
        self.scanned.store(scanned, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
    }

    pub fn mark_ready(&self) {
        self.ready.store(true, Ordering::Release);
        let _ = self.tx.send(true);
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    pub fn progress(&self) -> (usize, usize) {
        (
            self.scanned.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
        )
    }

    /// Resolve once the index is fully built. Returns immediately if it already is.
    pub async fn wait_ready(&self) {
        let mut rx = self.rx.clone();
        while !*rx.borrow_and_update() {
            if rx.changed().await.is_err() {
                return;
            }
        }
    }
}
