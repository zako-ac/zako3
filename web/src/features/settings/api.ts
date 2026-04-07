import { apiClient } from '@/lib/api-client'
import { apiCall } from '@/lib/api-helpers'
import type { UserSettings, PartialUserSettings } from './types'

export const settingsApi = {
    getSettings: async (): Promise<PartialUserSettings> =>
        apiCall(apiClient.get<PartialUserSettings>('/users/me/settings')),

    saveSettings: async (settings: PartialUserSettings): Promise<PartialUserSettings> =>
        apiCall(apiClient.put<PartialUserSettings>('/users/me/settings', settings)),

    getEffectiveSettings: async (guildId?: string): Promise<UserSettings> => {
        const query = guildId ? `?guild_id=${encodeURIComponent(guildId)}` : ''
        return apiCall(apiClient.get<UserSettings>(`/users/me/settings/effective${query}`))
    },

    // GuildUser scope
    getGuildUserSettings: async (guildId: string): Promise<PartialUserSettings> =>
        apiCall(apiClient.get<PartialUserSettings>(`/users/me/settings/guilds/${encodeURIComponent(guildId)}`)),

    saveGuildUserSettings: async (guildId: string, settings: PartialUserSettings): Promise<PartialUserSettings> =>
        apiCall(apiClient.put<PartialUserSettings>(`/users/me/settings/guilds/${encodeURIComponent(guildId)}`, settings)),

    deleteGuildUserSettings: async (guildId: string): Promise<void> =>
        apiCall(apiClient.delete<void>(`/users/me/settings/guilds/${encodeURIComponent(guildId)}`)),

    // Guild scope (admin)
    getGuildSettings: async (guildId: string): Promise<PartialUserSettings> =>
        apiCall(apiClient.get<PartialUserSettings>(`/guilds/${encodeURIComponent(guildId)}/settings`)),

    saveGuildSettings: async (guildId: string, settings: PartialUserSettings): Promise<PartialUserSettings> =>
        apiCall(apiClient.put<PartialUserSettings>(`/guilds/${encodeURIComponent(guildId)}/settings`, settings)),

    // Global scope (admin)
    getGlobalSettings: async (): Promise<PartialUserSettings> =>
        apiCall(apiClient.get<PartialUserSettings>('/settings/global')),

    saveGlobalSettings: async (settings: PartialUserSettings): Promise<PartialUserSettings> =>
        apiCall(apiClient.put<PartialUserSettings>('/settings/global', settings)),
}
