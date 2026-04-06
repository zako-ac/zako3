import { apiCall } from '@/lib/api-helpers'
import { apiClient } from '@/lib/api-client'
import type {
    EditQueueDto,
    GuildPlaybackStateDto,
    PlaybackActionDto,
    SkipDto,
    StopTrackDto,
} from '@zako-ac/zako3-data'

export const playbackApi = {
    getState: (): Promise<GuildPlaybackStateDto[]> =>
        apiCall(apiClient.get<GuildPlaybackStateDto[]>('/playback/state')),

    stopTrack: (body: StopTrackDto): Promise<PlaybackActionDto> =>
        apiCall(apiClient.post<PlaybackActionDto>('/playback/stop', body)),

    skipMusic: (body: SkipDto): Promise<PlaybackActionDto> =>
        apiCall(apiClient.post<PlaybackActionDto>('/playback/skip', body)),

    editQueue: (body: EditQueueDto): Promise<PlaybackActionDto> =>
        apiCall(apiClient.patch<PlaybackActionDto>('/playback/queue', body)),

    undoAction: (actionId: string): Promise<PlaybackActionDto> =>
        apiCall(apiClient.post<PlaybackActionDto>(`/playback/undo/${actionId}`)),

    getHistory: (): Promise<PlaybackActionDto[]> =>
        apiCall(apiClient.get<PlaybackActionDto[]>('/playback/history')),
}
