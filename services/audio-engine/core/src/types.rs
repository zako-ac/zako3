pub type BoxConsumer = Box<dyn ringbuf::traits::Consumer<Item = f32> + Send>;
pub type BoxProducer = Box<dyn ringbuf::traits::Producer<Item = f32> + Send>;

pub use zako3_types::*;
