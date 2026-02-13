use std::result::Result;
use std::sync::Arc;
use tonic::{Request, Response, Status};

use tracing::instrument;
use zako3_audio_engine_protos as proto;
use zako3_audio_engine_protos::audio_engine_server::AudioEngine;

use zako3_audio_engine_core::engine::session_manager::SessionManager;
use zako3_audio_engine_core::types::{
    AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName, TapName, TrackId, UserId,
    Volume,
};

pub mod config;

pub struct AudioEngineServer {
    pub session_manager: Arc<SessionManager>,
}

impl AudioEngineServer {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }
}

#[tonic::async_trait]
impl AudioEngine for AudioEngineServer {
    #[instrument(skip(self))]
    async fn join(
        &self,
        request: Request<proto::JoinRequest>,
    ) -> Result<Response<proto::OkResponse>, Status> {
        let req = request.into_inner();
        let guild_id: GuildId = GuildId::from(req.guild_id);
        let channel_id: ChannelId = ChannelId::from(req.channel_id);

        self.session_manager
            .join(guild_id, channel_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(proto::OkResponse {
            result: Some(proto::ok_response::Result::Success(true)),
        }))
    }

    #[instrument(skip(self))]
    async fn leave(
        &self,
        request: Request<proto::LeaveRequest>,
    ) -> Result<Response<proto::OkResponse>, Status> {
        let req = request.into_inner();
        let guild_id: GuildId = GuildId::from(req.guild_id);

        self.session_manager
            .leave(guild_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(proto::OkResponse {
            result: Some(proto::ok_response::Result::Success(true)),
        }))
    }

    #[instrument(skip(self))]
    async fn play(
        &self,
        request: Request<proto::PlayRequest>,
    ) -> Result<Response<proto::PlayResponse>, Status> {
        let req = request.into_inner();
        let guild_id: GuildId = GuildId::from(req.guild_id);

        let session = self
            .session_manager
            .get_session(guild_id)
            .ok_or_else(|| Status::not_found("Session not found"))?;

        let track_id = session
            .play(
                QueueName::from(req.queue_name),
                TapName::from(req.tap_name),
                AudioRequestString::from(req.audio_request_string),
                Volume::from(req.volume),
            )
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let id: u64 = track_id.into();
        Ok(Response::new(proto::PlayResponse {
            result: Some(proto::play_response::Result::TrackId(id)),
        }))
    }

    #[instrument(skip(self))]
    async fn set_volume(
        &self,
        request: Request<proto::SetVolumeRequest>,
    ) -> Result<Response<proto::OkResponse>, Status> {
        let req = request.into_inner();
        let guild_id: GuildId = GuildId::from(req.guild_id);

        let session = self
            .session_manager
            .get_session(guild_id)
            .ok_or_else(|| Status::not_found("Session not found"))?;

        session
            .set_volume(TrackId::from(req.track_id), Volume::from(req.volume))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(proto::OkResponse {
            result: Some(proto::ok_response::Result::Success(true)),
        }))
    }

    #[instrument(skip(self))]
    async fn stop(
        &self,
        request: Request<proto::StopRequest>,
    ) -> Result<Response<proto::OkResponse>, Status> {
        let req = request.into_inner();
        let guild_id: GuildId = GuildId::from(req.guild_id);

        let session = self
            .session_manager
            .get_session(guild_id)
            .ok_or_else(|| Status::not_found("Session not found"))?;

        let track_id_str = req.track_id;
        let track_id_u64 = track_id_str
            .parse::<u64>()
            .map_err(|_| Status::invalid_argument("Invalid track ID format"))?;

        session
            .stop(TrackId::from(track_id_u64))
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(proto::OkResponse {
            result: Some(proto::ok_response::Result::Success(true)),
        }))
    }

    #[instrument(skip(self))]
    async fn stop_many(
        &self,
        request: Request<proto::StopManyRequest>,
    ) -> Result<Response<proto::OkResponse>, Status> {
        let req = request.into_inner();
        let guild_id: GuildId = GuildId::from(req.guild_id);

        let session = self
            .session_manager
            .get_session(guild_id)
            .ok_or_else(|| Status::not_found("Session not found"))?;

        let filter_msg = req
            .filter
            .ok_or_else(|| Status::invalid_argument("Missing filter"))?;

        let filter = match filter_msg.filter_type {
            Some(proto::audio_stop_filter::FilterType::All(_)) => AudioStopFilter::All,
            Some(proto::audio_stop_filter::FilterType::Music(_)) => AudioStopFilter::Music,
            Some(proto::audio_stop_filter::FilterType::Tts(tts)) => {
                AudioStopFilter::TTS(UserId::from(tts.user_id))
            }
            None => return Err(Status::invalid_argument("Invalid stop filter type")),
        };

        session
            .stop_many(filter)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(proto::OkResponse {
            result: Some(proto::ok_response::Result::Success(true)),
        }))
    }

    #[instrument(skip(self))]
    async fn next_music(
        &self,
        request: Request<proto::NextMusicRequest>,
    ) -> Result<Response<proto::OkResponse>, Status> {
        let req = request.into_inner();
        let guild_id: GuildId = GuildId::from(req.guild_id);

        let session = self
            .session_manager
            .get_session(guild_id)
            .ok_or_else(|| Status::not_found("Session not found"))?;

        session
            .next_music()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(proto::OkResponse {
            result: Some(proto::ok_response::Result::Success(true)),
        }))
    }

    #[instrument(skip(self))]
    async fn get_session_state(
        &self,
        request: Request<proto::GetSessionStateRequest>,
    ) -> Result<Response<proto::SessionStateResponse>, Status> {
        let session = self
            .session_manager
            .get_session(request.get_ref().guild_id.into())
            .ok_or_else(|| Status::not_found("Session not found"))?;
        let state = session
            .session_state()
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("Session state not found"))?;

        let queues = state
            .queues
            .into_iter()
            .map(|(name, tracks)| proto::Queue {
                name: name.into(),
                tracks: tracks
                    .into_iter()
                    .map(|track| track_to_proto(track))
                    .collect(),
            })
            .collect();

        Ok(Response::new(proto::SessionStateResponse {
            result: Some(proto::session_state_response::Result::State(
                proto::SessionState {
                    guild_id: request.into_inner().guild_id,
                    channel_id: state.channel_id.into(),
                    queues,
                },
            )),
        }))
    }
}

fn track_to_proto(track: zako3_audio_engine_core::types::Track) -> proto::Track {
    proto::Track {
        track_id: track.track_id.into(),
        description: track.description.into(),
        queue_name: track.queue_name.into(),
        audio_request_string: track.request.audio_request.into(),
        cache_key: track.request.cache_key.into(),
        tap_name: track.request.tap_name.into(),
        volume: track.volume.into(),
    }
}
