use async_nats::Client;
use opentelemetry::global;
use std::time::Duration;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use zako3_audio_engine_core::types::{
    AudioRequestString, AudioStopFilter, ChannelId, GuildId, QueueName, SessionState, TrackId,
    Volume, hq::{DiscordUserId, TapId},
};

use crate::{AudioEngineRequest, AudioEngineResponse, TracedAudioEngineRequest};

pub struct AudioEngineRpcClient {
    client: Client,
}

impl AudioEngineRpcClient {
    pub async fn new(nats_url: &str) -> anyhow::Result<Self> {
        let client = async_nats::connect(nats_url).await?;
        Ok(Self { client })
    }

    async fn send_request(
        &self,
        subject: &str,
        request: AudioEngineRequest,
    ) -> anyhow::Result<AudioEngineResponse> {
        let mut trace_headers = std::collections::HashMap::new();
        let cx = tracing::Span::current().context();
        global::get_text_map_propagator(|p| p.inject_context(&cx, &mut trace_headers));

        let traced = TracedAudioEngineRequest { inner: request, trace_headers };
        let payload = serde_json::to_vec(&traced)?;

        let msg = tokio::time::timeout(
            Duration::from_secs(10),
            self.client.request(subject.to_string(), payload.into()),
        )
        .await
        .map_err(|_| anyhow::anyhow!("audio engine request timed out"))??;
        Ok(serde_json::from_slice(&msg.payload)?)
    }

    pub async fn join(&self, guild_id: GuildId, channel_id: ChannelId) -> anyhow::Result<bool> {
        match self
            .send_request(
                "audio_engine.control",
                AudioEngineRequest::Join {
                    guild_id,
                    channel_id,
                },
            )
            .await?
        {
            AudioEngineResponse::SuccessBool(b) => Ok(b),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn leave(&self, guild_id: GuildId, channel_id: ChannelId) -> anyhow::Result<bool> {
        match self
            .send_request(
                "audio_engine.control",
                AudioEngineRequest::Leave {
                    guild_id,
                    channel_id,
                },
            )
            .await?
        {
            AudioEngineResponse::SuccessBool(b) => Ok(b),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn play(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        queue_name: QueueName,
        tap_id: TapId,
        audio_request_string: AudioRequestString,
        volume: Volume,
        discord_user_id: DiscordUserId,
    ) -> anyhow::Result<TrackId> {
        let subject = format!("audio_engine.session.{}.{}", guild_id, channel_id);
        match self
            .send_request(
                &subject,
                AudioEngineRequest::Play {
                    guild_id,
                    queue_name,
                    tap_id,
                    audio_request_string,
                    volume,
                    discord_user_id,
                },
            )
            .await?
        {
            AudioEngineResponse::SuccessTrackId(id) => Ok(id),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn set_volume(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        track_id: TrackId,
        volume: Volume,
    ) -> anyhow::Result<bool> {
        let subject = format!("audio_engine.session.{}.{}", guild_id, channel_id);
        match self
            .send_request(
                &subject,
                AudioEngineRequest::SetVolume {
                    guild_id,
                    track_id,
                    volume,
                },
            )
            .await?
        {
            AudioEngineResponse::SuccessBool(b) => Ok(b),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn stop(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        track_id: TrackId,
    ) -> anyhow::Result<bool> {
        let subject = format!("audio_engine.session.{}.{}", guild_id, channel_id);
        match self
            .send_request(&subject, AudioEngineRequest::Stop { guild_id, track_id })
            .await?
        {
            AudioEngineResponse::SuccessBool(b) => Ok(b),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn stop_many(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        filter: AudioStopFilter,
    ) -> anyhow::Result<bool> {
        let subject = format!("audio_engine.session.{}.{}", guild_id, channel_id);
        match self
            .send_request(&subject, AudioEngineRequest::StopMany { guild_id, filter })
            .await?
        {
            AudioEngineResponse::SuccessBool(b) => Ok(b),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn next_music(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> anyhow::Result<bool> {
        let subject = format!("audio_engine.session.{}.{}", guild_id, channel_id);
        match self
            .send_request(&subject, AudioEngineRequest::NextMusic { guild_id })
            .await?
        {
            AudioEngineResponse::SuccessBool(b) => Ok(b),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn pause(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        track_id: TrackId,
    ) -> anyhow::Result<bool> {
        let subject = format!("audio_engine.session.{}.{}", guild_id, channel_id);
        match self
            .send_request(&subject, AudioEngineRequest::Pause { guild_id, track_id })
            .await?
        {
            AudioEngineResponse::SuccessBool(b) => Ok(b),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn resume(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        track_id: TrackId,
    ) -> anyhow::Result<bool> {
        let subject = format!("audio_engine.session.{}.{}", guild_id, channel_id);
        match self
            .send_request(&subject, AudioEngineRequest::Resume { guild_id, track_id })
            .await?
        {
            AudioEngineResponse::SuccessBool(b) => Ok(b),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn get_session_state(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> anyhow::Result<SessionState> {
        let subject = format!("audio_engine.session.{}.{}", guild_id, channel_id);
        match self
            .send_request(&subject, AudioEngineRequest::GetSessionState { guild_id })
            .await?
        {
            AudioEngineResponse::SuccessSessionState(s) => Ok(s),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }

    pub async fn get_sessions_in_guild(
        &self,
        guild_id: GuildId,
    ) -> anyhow::Result<Vec<SessionState>> {
        match self
            .send_request(
                "audio_engine.control",
                AudioEngineRequest::GetSessionsInGuild { guild_id },
            )
            .await?
        {
            AudioEngineResponse::SuccessSessions(s) => Ok(s),
            AudioEngineResponse::Error(e) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Invalid response type")),
        }
    }
}
