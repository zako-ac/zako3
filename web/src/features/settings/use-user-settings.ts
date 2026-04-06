import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { settingsApi } from './api'
import type { UserSettings } from './types'

export const settingsKeys = {
    settings: () => ['userSettings'] as const,
}

export const useUserSettings = () =>
    useQuery({
        queryKey: settingsKeys.settings(),
        queryFn: settingsApi.getSettings,
    })

export const useSaveUserSettings = () => {
    const queryClient = useQueryClient()
    return useMutation<UserSettings, Error, UserSettings>({
        mutationFn: settingsApi.saveSettings,
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: settingsKeys.settings() })
        },
    })
}
