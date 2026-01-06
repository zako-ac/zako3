import type { AudioRequestString, AudioStopFilter, ChannelId, GuildId, TapId, TrackId, Volume } from "./types.js";

export interface ZakoService {
    join(channelId: ChannelId): Promise<void>;
    leave(): Promise<void>;

    play(tapId: TapId, audioRequest: AudioRequestString, volume: Volume): Promise<TrackId>;
    setVolume(trackId: TrackId, volume: Volume): Promise<void>;

    stop(trackId: TrackId): Promise<void>;
    stopMany(filter: AudioStopFilter): Promise<void>;

    nextMusic(): Promise<void>;

    setPaused(paused: boolean): Promise<void>;
}

export function createZakoService(_guildId: GuildId) { }
