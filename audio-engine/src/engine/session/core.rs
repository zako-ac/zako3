use crossbeam::channel::Receiver;
use ringbuf::traits::{Consumer, Producer};

use crate::{
    codec::decoder::Decoder,
    engine::mixer::{Mixer, create_mixer},
    service::StateService,
    types::SessionControlCommand,
};

pub struct SessionCore<D, SS>
where
    SS: StateService,
    D: Decoder,
{
    state_service: SS,
    decoder: D,
}

fn session_core_thread<C>(
    session_core: SessionCore<impl Decoder, impl StateService>,
    cmd_rx: Receiver<SessionControlCommand>,
    output: impl Producer<Item = f32> + Send + 'static,
) where
    C: Consumer<Item = f32> + Send + 'static,
{
    let (end_tx, end_rx) = crossbeam::channel::unbounded();
    let mixer: Mixer<C> = create_mixer(output);

    let states = session_core.state_service;

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                SessionControlCommand::Play(audio_request) => {}
                _ => {}
            }
        }

        while let Ok(finished_track_id) = end_rx.try_recv() {
            mixer.remove_source(finished_track_id);
        }
    }
}
