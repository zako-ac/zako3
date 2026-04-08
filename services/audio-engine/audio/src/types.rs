use bytes::Bytes;
use crossbeam::channel::{Receiver, Sender};
use ringbuf::{HeapCons, HeapProd};
pub use zako3_types::*;

use crate::BUFFER_SIZE;

pub type PCMSample = [f32; BUFFER_SIZE];

pub type BoxConsumer = Receiver<PCMSample>;
pub type BoxProducer = Sender<PCMSample>;

pub type RingProd = HeapProd<f32>;
pub type RingCons = HeapCons<f32>;

pub type OpusProd = HeapProd<Bytes>;
pub type OpusCons = HeapCons<Bytes>;

pub type PCMSender = tokio::sync::mpsc::Sender<PCMSample>;
pub type PCMReceiver = tokio::sync::mpsc::Receiver<PCMSample>;
