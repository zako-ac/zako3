use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// A simple Snowflake-like ID generator.
/// Format: 42 bits timestamp (ms), 10 bits machine/node id, 12 bits sequence.
pub struct IdGenerator {
    node_id: u64,
    last_timestamp: AtomicU64,
    sequence: AtomicU64,
}

impl IdGenerator {
    pub const fn new(node_id: u64) -> Self {
        Self {
            node_id: node_id & 0x3FF, // 10 bits
            last_timestamp: AtomicU64::new(0),
            sequence: AtomicU64::new(0),
        }
    }

    pub fn next_id(&self) -> u64 {
        loop {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis() as u64;

            let last = self.last_timestamp.load(Ordering::Relaxed);

            let epoch = 1704067200000;
            let ts = now.saturating_sub(epoch);

            if ts > last {
                if self
                    .last_timestamp
                    .compare_exchange(last, ts, Ordering::SeqCst, Ordering::Relaxed)
                    .is_ok()
                {
                    self.sequence.store(0, Ordering::SeqCst);
                    return self.format_id(ts, 0);
                }
            } else if ts == last {
                let seq = self.sequence.fetch_add(1, Ordering::SeqCst) + 1;
                if seq < 4096 {
                    return self.format_id(ts, seq);
                }
                // Sequence overflow, spin until next millisecond
                std::thread::yield_now();
            } else {
                // Clock moved backwards
                std::thread::yield_now();
            }
        }
    }

    fn format_id(&self, ts: u64, sequence: u64) -> u64 {
        ((ts & 0x3FFFFFFFFFF) << 22) | (self.node_id << 12) | (sequence & 0xFFF)
    }
}

pub static GLOBAL_ID_GEN: IdGenerator = IdGenerator::new(1);

pub fn next_id() -> u64 {
    GLOBAL_ID_GEN.next_id()
}
