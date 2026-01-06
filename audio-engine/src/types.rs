use std::collections::HashMap;

use derive_more::{From, FromStr, Into};
use serde::{Deserialize, Serialize};

use crate::constant::BUFFER_SIZE;

pub type AudioBuffer = [f32; BUFFER_SIZE];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Into, From, Serialize, Deserialize)]
pub struct GuildId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Into, From, Serialize, Deserialize)]
pub struct ChannelId(u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Into, From, FromStr, Serialize, Deserialize)]
pub struct QueueName(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Into, From, Serialize, Deserialize)]
pub struct TrackId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Into, From, Serialize, Deserialize)]
pub struct UserId(u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Into, From, FromStr, Serialize, Deserialize)]
pub struct TapName(String);

#[derive(Debug, Clone, Copy, PartialEq, Into, From, Serialize, Deserialize)]
pub struct Volume(f32);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Into, From, FromStr, Serialize, Deserialize)]
pub struct AudioRequestString(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Into, From, FromStr, Serialize, Deserialize)]
pub struct StreamCacheKey(String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioStopFilter {
    All,
    Music,
    TTS(UserId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRequest {
    pub tap_name: TapName,
    pub request: AudioRequestString,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub guild_id: GuildId,
    pub channel_id: ChannelId,
    pub queues: HashMap<QueueName, Vec<Track>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub track_id: TrackId,
    pub request: AudioRequest,
    pub volume: Volume,
    pub queue_name: QueueName,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionControlCommand {
    Play(AudioRequest),
    SetVolume(TrackId, Volume),
    Stop(TrackId),
    StopMany(AudioStopFilter),
    NextMusic,
    SetPaused(bool),
}

impl SessionState {
    pub fn find_track_mut(&mut self, track_id: TrackId) -> Option<&mut Track> {
        for queue in self.queues.values_mut() {
            if let Some(track) = queue.iter_mut().find(|t| t.track_id == track_id) {
                return Some(track);
            }
        }
        None
    }

    pub fn remove_track(&mut self, track_id: TrackId) {
        for queue in self.queues.values_mut() {
            queue.retain(|t| t.track_id != track_id);
        }
    }

    pub fn get_all_track_ids(&self) -> Vec<TrackId> {
        let mut track_ids = Vec::new();
        for queue in self.queues.values() {
            for track in queue {
                track_ids.push(track.track_id);
            }
        }
        track_ids
    }

    pub fn get_all_track_ids_by_queue_name_prefix(&self, prefix: &str) -> Vec<TrackId> {
        let mut track_ids = Vec::new();
        for (queue_name, queue) in &self.queues {
            if queue_name.0.starts_with(prefix) {
                for track in queue {
                    track_ids.push(track.track_id);
                }
            }
        }
        track_ids
    }

    pub fn find_track(&self, track_id: TrackId) -> Option<&Track> {
        for queue in self.queues.values() {
            if let Some(track) = queue.iter().find(|t| t.track_id == track_id) {
                return Some(track);
            }
        }
        None
    }

    pub fn get_active_tracks(&self) -> Vec<Track> {
        let mut tracks = Vec::new();
        for queue in self.queues.values() {
            // first ones
            if let Some(track) = queue.first() {
                tracks.push(track.clone());
            }
        }
        tracks
    }
}
