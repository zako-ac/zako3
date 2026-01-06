use crossbeam::channel::Sender;
use ringbuf::traits::Consumer;
use tokio::io::AsyncRead;

use crate::{error::ZakoResult, types::TrackId};

pub trait Decoder {
    fn start_decoding<C: Consumer<Item = f32> + Send + 'static>(
        &self,
        track_id: TrackId,
        stream: impl AsyncRead,
        end_tx: Sender<TrackId>,
    ) -> ZakoResult<C>;
}
