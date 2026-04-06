import { apiClient } from '@/lib/api-client'
import { apiCall } from '@/lib/api-helpers'
import type { UserSettings } from './types'

export const settingsApi = {
    getSettings: async (): Promise<UserSettings> =>
        apiCall(apiClient.get<UserSettings>('/users/me/settings')),

    saveSettings: async (settings: UserSettings): Promise<UserSettings> =>
        apiCall(apiClient.put<UserSettings>('/users/me/settings', settings)),
}
