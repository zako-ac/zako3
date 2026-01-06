use crossbeam::channel::Sender;
use mockall::automock;
use tokio::io::AsyncRead;

use crate::{
    error::ZakoResult,
    types::{BoxConsumer, TrackId},
};

#[automock]
pub trait Decoder {
    fn start_decoding(
        &self,
        track_id: TrackId,
        stream: Box<dyn AsyncRead + Unpin + Send>,
        end_tx: Sender<TrackId>,
    ) -> ZakoResult<BoxConsumer>;
}
