use zako3_types::{ChannelId, GuildId, hq::DiscordUserId};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VoiceState {
    pub voice_channel_id: ChannelId,
    pub mute: bool,
    pub deaf: bool,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VoiceStateUpdateEvent {
    pub guild_id: GuildId,
    pub user_id: DiscordUserId,
    pub before: Option<VoiceState>,
    pub after: Option<VoiceState>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum StateChangeEvent {
    VoiceStateUpdate(VoiceStateUpdateEvent),
}
