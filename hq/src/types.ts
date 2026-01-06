export type Brand<T, B> = T & { __brand: B }

export type GuildId = Brand<string, 'GuildId'>
export type ChannelId = Brand<string, 'ChannelId'>

export type QueueId = Brand<string, 'QueueId'>
export type QueueName = Brand<string, 'QueueName'>

export type TrackId = Brand<string, 'TrackId'>
export type TapId = Brand<string, 'TapId'>
export type Volume = Brand<number, 'Volume'>

export type AudioRequestString = Brand<string, 'AudioRequestString'>

export type Track = {
    id: TrackId,
    volume: Volume,
    audioRequest: AudioRequestString,
}

export type Queue = {
    id: QueueId,
    name: QueueName,
    tracks: Track[],
}

export type Session = {
    guildId: GuildId,
    channelId: ChannelId,
    queues: Queue[],
}

export type TapRole = 'music' | 'tts'

export type AudioStopFilter = TapRole | 'all'
