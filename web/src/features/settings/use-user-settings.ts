import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { settingsApi } from './api'
import type { UserSettings, PartialUserSettings } from './types'
import { toPartial, emptyPartial } from './types'

export const settingsKeys = {
    settings: () => ['userSettings'] as const,
    effectiveSettings: (guildId?: string) => ['effectiveSettings', guildId] as const,
    guildUserSettings: (guildId: string) => ['guildUserSettings', guildId] as const,
    guildSettings: (guildId: string) => ['guildSettings', guildId] as const,
    globalSettings: () => ['globalSettings'] as const,
}

// Returns resolved (concrete) UserSettings — used where full concrete values are needed
export const useUserSettings = () =>
    useQuery({
        queryKey: settingsKeys.effectiveSettings(),
        queryFn: () => settingsApi.getEffectiveSettings(),
    })

export const useSaveUserSettings = () => {
    const queryClient = useQueryClient()
    return useMutation<PartialUserSettings, Error, UserSettings>({
        mutationFn: (settings) => settingsApi.saveSettings(toPartial(settings)),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: settingsKeys.effectiveSettings() })
            queryClient.invalidateQueries({ queryKey: settingsKeys.settings() })
        },
    })
}

// Returns user-scope PartialUserSettings — for the settings editor UI
export const usePartialUserSettings = () =>
    useQuery({
        queryKey: settingsKeys.settings(),
        queryFn: () => settingsApi.getSettings(),
    })

export const useSavePartialUserSettings = () => {
    const queryClient = useQueryClient()
    return useMutation<PartialUserSettings, Error, PartialUserSettings>({
        mutationFn: (settings) => settingsApi.saveSettings(settings),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: settingsKeys.settings() })
            queryClient.invalidateQueries({ queryKey: settingsKeys.effectiveSettings() })
        },
    })
}

// --- GuildUser scope ---

export const useGuildUserSettings = (guildId: string) =>
    useQuery({
        queryKey: settingsKeys.guildUserSettings(guildId),
        queryFn: () => settingsApi.getGuildUserSettings(guildId),
        enabled: !!guildId,
    })

export const useSaveGuildUserSettings = (guildId: string) => {
    const queryClient = useQueryClient()
    return useMutation<PartialUserSettings, Error, PartialUserSettings>({
        mutationFn: (settings) => settingsApi.saveGuildUserSettings(guildId, settings),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: settingsKeys.guildUserSettings(guildId) })
            queryClient.invalidateQueries({ queryKey: settingsKeys.effectiveSettings(guildId) })
        },
    })
}

export const useDeleteGuildUserSettings = (guildId: string) => {
    const queryClient = useQueryClient()
    return useMutation<void, Error, void>({
        mutationFn: () => settingsApi.deleteGuildUserSettings(guildId),
        onSuccess: () => {
            queryClient.setQueryData(settingsKeys.guildUserSettings(guildId), emptyPartial)
            queryClient.invalidateQueries({ queryKey: settingsKeys.effectiveSettings(guildId) })
        },
    })
}

// --- Guild scope ---

export const useGuildSettings = (guildId: string) =>
    useQuery({
        queryKey: settingsKeys.guildSettings(guildId),
        queryFn: () => settingsApi.getGuildSettings(guildId),
        enabled: !!guildId,
    })

export const useSaveGuildSettings = (guildId: string) => {
    const queryClient = useQueryClient()
    return useMutation<PartialUserSettings, Error, PartialUserSettings>({
        mutationFn: (settings) => settingsApi.saveGuildSettings(guildId, settings),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: settingsKeys.guildSettings(guildId) })
            queryClient.invalidateQueries({ queryKey: settingsKeys.effectiveSettings(guildId) })
        },
    })
}

// --- Global scope ---

export const useGlobalSettings = () =>
    useQuery({
        queryKey: settingsKeys.globalSettings(),
        queryFn: () => settingsApi.getGlobalSettings(),
    })

export const useSaveGlobalSettings = () => {
    const queryClient = useQueryClient()
    return useMutation<PartialUserSettings, Error, PartialUserSettings>({
        mutationFn: (settings) => settingsApi.saveGlobalSettings(settings),
        onSuccess: () => {
            queryClient.invalidateQueries({ queryKey: settingsKeys.globalSettings() })
        },
    })
}
