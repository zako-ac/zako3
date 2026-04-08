pub const BUFFER_SIZE: usize = 960 * CHANNELS as usize;
pub const RINGBUFFER_SIZE: usize = BUFFER_SIZE * 8;
pub const SAMPLE_RATE: u32 = 48000;
pub const CHANNELS: u32 = 2;

pub fn frame_duration() -> std::time::Duration {
    const SECS: f64 = BUFFER_SIZE as f64 / SAMPLE_RATE as f64 / CHANNELS as f64;
    std::time::Duration::from_secs_f64(SECS)
}
