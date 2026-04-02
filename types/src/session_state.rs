use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{ChannelId, GuildId, QueueName, Track, TrackId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub guild_id: GuildId,
    pub channel_id: ChannelId,
    pub queues: HashMap<QueueName, Vec<Track>>,
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

    pub fn get_all_track_ids_by_queue_name(&self, queue_name: &QueueName) -> Vec<TrackId> {
        self.queues
            .get(queue_name)
            .map(|queue| queue.iter().map(|track| track.track_id).collect())
            .unwrap_or_else(Vec::new)
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
