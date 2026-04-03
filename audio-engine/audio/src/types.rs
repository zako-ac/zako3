use bytes::Bytes;
use crossbeam::channel::{Receiver, Sender};
use ringbuf::{HeapCons, HeapProd};
pub use zako3_types::*;

use crate::BUFFER_SIZE;

pub type PCMSample = [i16; BUFFER_SIZE];

pub type BoxConsumer = Receiver<PCMSample>;
pub type BoxProducer = Sender<PCMSample>;

pub type RingProd = HeapProd<i16>;
pub type RingCons = HeapCons<i16>;

pub type OpusProd = HeapProd<Bytes>;
pub type OpusCons = HeapCons<Bytes>;

pub type PCMSender = tokio::sync::mpsc::Sender<PCMSample>;
pub type PCMReceiver = tokio::sync::mpsc::Receiver<PCMSample>;
