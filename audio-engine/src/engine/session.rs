use std::collections::HashMap;

use crossbeam::channel::Sender;

use crate::{
    codec::decoder::Decoder,
    engine::mixer::Mixer,
    error::ZakoResult,
    service::{StateService, TapHubService},
    types::{
        AudioRequest, AudioRequestString, AudioStopFilter, GuildId, QueueName, TapName, Track,
        TrackId, Volume,
    },
    util::id_gen,
};

pub struct SessionControl<SS, THS, M, D>
where
    SS: StateService,
    THS: TapHubService,
    M: Mixer,
    D: Decoder,
{
    pub guild_id: GuildId,

    pub(crate) mixer: M,
    pub(crate) decoder: D,

    pub(crate) end_tx: Sender<TrackId>,

    pub(crate) state_service: SS,
    pub(crate) taphub_service: THS,
}

impl<SS, THS, M, D> SessionControl<SS, THS, M, D>
where
    SS: StateService,
    THS: TapHubService,
    M: Mixer,
    D: Decoder,
{
    pub async fn play(
        &self,
        queue_name: QueueName,
        tap_name: TapName,
        request: AudioRequestString,
        volume: Volume,
    ) -> ZakoResult<TrackId> {
        let track_id: TrackId = id_gen::generate_id();

        self.state_service
            .modify_session(self.guild_id, move |session| {
                let track = Track {
                    track_id,
                    request: AudioRequest {
                        tap_name: tap_name.clone(),
                        request: request.clone(),
                    },
                    volume,
                    queue_name: queue_name.clone(),
                };

                upsert_track(&mut session.queues, queue_name.clone(), track);
            })
            .await?;

        self.reconcile().await?;

        Ok(track_id)
    }

    pub async fn set_volume(&self, track_id: TrackId, volume: Volume) -> ZakoResult<()> {
        self.mixer.set_volume(track_id, volume.into());
        self.state_service
            .modify_session(self.guild_id, move |session| {
                if let Some(track) = session.find_track_mut(track_id) {
                    track.volume = volume;
                }
            })
            .await?;
        Ok(())
    }

    pub async fn stop(&self, track_id: TrackId) -> ZakoResult<()> {
        self.mixer.remove_source(track_id);
        self.state_service
            .modify_session(self.guild_id, move |session| {
                session.remove_track(track_id);
            })
            .await?;
        Ok(())
    }

    pub async fn stop_many(&self, filter: AudioStopFilter) -> ZakoResult<()> {
        let mut session = self.state_service.get_session(self.guild_id).await?;
        let track_ids = session
            .as_ref()
            .map(|s| match filter {
                AudioStopFilter::All => s.get_all_track_ids(),
                AudioStopFilter::Music => s.get_all_track_ids_by_queue_name_prefix("music"),
                AudioStopFilter::TTS(user_id) => {
                    s.get_all_track_ids_by_queue_name_prefix(&format!("tts_{}", u64::from(user_id)))
                }
            })
            .unwrap_or_default();

        for track_id in track_ids {
            self.mixer.remove_source(track_id);

            if let Some(session) = session.as_mut() {
                session.remove_track(track_id);
            }
        }

        if let Some(session) = session {
            self.state_service.save_session(&session).await?;
        }

        Ok(())
    }

    pub async fn next_music(&self) -> ZakoResult<()> {
        let music_tracks = self
            .state_service
            .get_session(self.guild_id)
            .await?
            .map(|s| {
                s.get_all_track_ids_by_queue_name_prefix("music")
                    .into_iter()
                    .filter_map(|tid| s.find_track(tid).cloned())
                    .collect::<Vec<Track>>()
            })
            .unwrap_or_default();

        // two tracks: current and next
        if music_tracks.len() < 2 {
            return Ok(());
        }

        let current_track_id = music_tracks[0].track_id;
        self.mixer.remove_source(current_track_id);
        self.state_service
            .modify_session(self.guild_id, move |session| {
                session.remove_track(current_track_id);
            })
            .await?;

        self.reconcile().await?;

        Ok(())
    }

    /// Reconcile the session state with the mixer state
    async fn reconcile(&self) -> ZakoResult<()> {
        let session = self.state_service.get_session(self.guild_id).await?;

        if let Some(session) = session {
            let active_tracks = session.get_active_tracks();

            for track in active_tracks {
                if !self.mixer.has_source(track.track_id).await {
                    self.play_now(track).await?;
                }
            }
        }

        Ok(())
    }

    pub async fn set_paused(&self, paused: bool) -> ZakoResult<()> {
        todo!()
    }

    async fn play_now(&self, track: Track) -> ZakoResult<()> {
        let stream = self
            .taphub_service
            .request_audio(track.request.clone())
            .await?;

        let consumer = self
            .decoder
            .start_decoding(track.track_id, stream, self.end_tx.clone())?;

        self.mixer.add_source(track.track_id, consumer);
        self.mixer.set_volume(track.track_id, track.volume.into());

        Ok(())
    }
}

fn upsert_track(queues: &mut HashMap<QueueName, Vec<Track>>, queue_name: QueueName, track: Track) {
    if let Some(queue) = queues.get_mut(&queue_name) {
        queue.push(track);
    } else {
        queues.insert(queue_name.clone(), vec![track]);
    }
}
