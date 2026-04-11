import { useEffect } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { playbackApi } from './api'
import type { EditQueueDto, PauseTrackDto, ResumeTrackDto, SkipDto, StopTrackDto } from '@zako-ac/zako3-data'
import { AUTH_TOKEN_KEY, API_BASE_URL } from '@/lib/constants'

export const playbackKeys = {
    all: ['playback'] as const,
    state: () => [...playbackKeys.all, 'state'] as const,
    history: () => [...playbackKeys.all, 'history'] as const,
}

export const usePlaybackState = () =>
    useQuery({
        queryKey: playbackKeys.state(),
        queryFn: playbackApi.getState,
        refetchInterval: 30_000,
    })

export const useRefreshPlaybackState = () => {
    const queryClient = useQueryClient()
    return () => queryClient.invalidateQueries({ queryKey: playbackKeys.state() })
}

export const usePlaybackSSE = () => {
    const queryClient = useQueryClient()
    useEffect(() => {
        const token = localStorage.getItem(AUTH_TOKEN_KEY) ?? ''
        const url = `${API_BASE_URL}/playback/sse?token=${encodeURIComponent(token)}`
        const es = new EventSource(url)
        es.onmessage = () => {
            queryClient.invalidateQueries({ queryKey: playbackKeys.state() })
        }
        es.onerror = () => es.close()
        return () => es.close()
    }, [queryClient])
}

export const usePlaybackHistory = () =>
    useQuery({
        queryKey: playbackKeys.history(),
        queryFn: playbackApi.getHistory,
    })

export const useStopTrack = () => {
    const queryClient = useQueryClient()
    return useMutation({
        mutationFn: (body: StopTrackDto) => playbackApi.stopTrack(body),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: playbackKeys.state() })
            queryClient.invalidateQueries({ queryKey: playbackKeys.history() })
        },
    })
}

export const useSkipMusic = () => {
    const queryClient = useQueryClient()
    return useMutation({
        mutationFn: (body: SkipDto) => playbackApi.skipMusic(body),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: playbackKeys.state() })
            queryClient.invalidateQueries({ queryKey: playbackKeys.history() })
        },
    })
}

export const usePauseTrack = () => {
    const queryClient = useQueryClient()
    return useMutation({
        mutationFn: (body: PauseTrackDto) => playbackApi.pauseTrack(body),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: playbackKeys.state() })
            queryClient.invalidateQueries({ queryKey: playbackKeys.history() })
        },
    })
}

export const useResumeTrack = () => {
    const queryClient = useQueryClient()
    return useMutation({
        mutationFn: (body: ResumeTrackDto) => playbackApi.resumeTrack(body),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: playbackKeys.state() })
            queryClient.invalidateQueries({ queryKey: playbackKeys.history() })
        },
    })
}

export const useEditQueue = () => {
    const queryClient = useQueryClient()
    return useMutation({
        mutationFn: (body: EditQueueDto) => playbackApi.editQueue(body),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: playbackKeys.state() })
            queryClient.invalidateQueries({ queryKey: playbackKeys.history() })
        },
    })
}

export const useUndoAction = () => {
    const queryClient = useQueryClient()
    return useMutation({
        mutationFn: (actionId: string) => playbackApi.undoAction(actionId),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: playbackKeys.state() })
            queryClient.invalidateQueries({ queryKey: playbackKeys.history() })
        },
    })
}
