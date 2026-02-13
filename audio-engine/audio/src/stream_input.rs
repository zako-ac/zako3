use std::io::{self, ErrorKind, Read, Seek, SeekFrom};

use parking_lot::Mutex;
use ringbuf::traits::Consumer;
use songbird::input::RawAdapter;
use symphonia::core::io::ReadOnlySource;
use zako3_audio_engine_types::ZakoResult;

use crate::{BUFFER_SIZE, CHANNELS, RingCons, SAMPLE_RATE};

pub fn create_sync_stream_input(
    consumer: RingCons,
) -> ZakoResult<RawAdapter<ReadOnlySource<SyncStreamInput>>> {
    let stream = ReadOnlySource::new(SyncStreamInput {
        consumer: consumer.into(),
    });

    let adapter = RawAdapter::new(stream, SAMPLE_RATE, CHANNELS);

    Ok(adapter)
}

pub struct SyncStreamInput {
    consumer: Mutex<RingCons>,
}

impl Read for SyncStreamInput {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut temp_buf = [0f32; BUFFER_SIZE];
        self.consumer.lock().pop_slice(&mut temp_buf);

        let byte_buf = bytemuck::cast_slice(&temp_buf);
        let len = byte_buf.len().min(buf.len());
        buf[..len].copy_from_slice(&byte_buf[..len]);
        Ok(len)
    }
}

impl Seek for SyncStreamInput {
    fn seek(&mut self, _pos: SeekFrom) -> io::Result<u64> {
        Err(ErrorKind::Unsupported.into())
    }
}

#[cfg(test)]
mod tests {
    use ringbuf::traits::{Producer, Split};

    use super::*;

    #[test]
    fn sync_stream_io() {
        let (mut prod, cons) = ringbuf::HeapRb::<f32>::new(BUFFER_SIZE).split();

        let mut input = create_sync_stream_input(cons).unwrap();

        let slice = [0.5f32; BUFFER_SIZE];
        prod.push_slice(&slice);

        let mut buf = [0u8; BUFFER_SIZE * 4];
        input.read_exact(&mut buf).unwrap();
    }
}
