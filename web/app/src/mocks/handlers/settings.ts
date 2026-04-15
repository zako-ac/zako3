import { http, HttpResponse, delay } from 'msw'
import { API_BASE } from './base'
import type { PartialUserSettings, UserSettings } from '@/features/settings/types'
import { emptyPartial, defaultUserSettings } from '@/features/settings/types'

const mockSettingsStore: Record<string, PartialUserSettings> = {
  global: emptyPartial,
}

const defaultEffectiveSettings: UserSettings = {
  text_mappings: [],
  emoji_mappings: [],
  text_reading_rule: 'always',
  user_join_leave_alert: { type: 'auto' },
  max_message_length: 100,
  enable_tts_queue: true,
  tts_voice: null,
}

export const settingsHandlers = [
  // User scope settings
  http.get(`${API_BASE}/users/me/settings`, async () => {
    await delay(100)
    return HttpResponse.json(emptyPartial)
  }),

  http.put(`${API_BASE}/users/me/settings`, async ({ request }) => {
    await delay(150)
    const body = (await request.json()) as PartialUserSettings
    // In real app, this would merge and persist. For mock, just return the input.
    return HttpResponse.json(body)
  }),

  http.get(`${API_BASE}/users/me/settings/effective`, async () => {
    await delay(100)
    return HttpResponse.json(defaultEffectiveSettings)
  }),

  // Guild-user scope settings
  http.get(`${API_BASE}/users/me/settings/guilds/:guildId`, async () => {
    await delay(100)
    return HttpResponse.json(emptyPartial)
  }),

  http.put(`${API_BASE}/users/me/settings/guilds/:guildId`, async ({ request }) => {
    await delay(150)
    const body = (await request.json()) as PartialUserSettings
    return HttpResponse.json(body)
  }),

  http.delete(`${API_BASE}/users/me/settings/guilds/:guildId`, async () => {
    await delay(100)
    return new HttpResponse(null, { status: 204 })
  }),

  // Guild scope settings
  http.get(`${API_BASE}/guilds/:guildId/settings`, async () => {
    await delay(100)
    return HttpResponse.json(emptyPartial)
  }),

  http.put(`${API_BASE}/guilds/:guildId/settings`, async ({ request }) => {
    await delay(150)
    const body = (await request.json()) as PartialUserSettings
    return HttpResponse.json(body)
  }),

  // Global scope settings (admin)
  http.get(`${API_BASE}/settings/global`, async () => {
    await delay(100)
    return HttpResponse.json(mockSettingsStore.global || defaultUserSettings)
  }),

  http.put(`${API_BASE}/settings/global`, async ({ request }) => {
    await delay(150)
    const body = (await request.json()) as PartialUserSettings
    mockSettingsStore.global = { ...mockSettingsStore.global, ...body }
    return HttpResponse.json(mockSettingsStore.global)
  }),

  // Admin endpoints
  http.get(`${API_BASE}/admin/users/:userId/settings`, async () => {
    await delay(100)
    return HttpResponse.json(emptyPartial)
  }),

  http.put(`${API_BASE}/admin/users/:userId/settings`, async ({ request }) => {
    await delay(150)
    const body = (await request.json()) as PartialUserSettings
    return HttpResponse.json(body)
  }),

  http.get(`${API_BASE}/admin/users/:userId/settings/guilds/:guildId`, async () => {
    await delay(100)
    return HttpResponse.json(emptyPartial)
  }),

  http.put(`${API_BASE}/admin/users/:userId/settings/guilds/:guildId`, async ({ request }) => {
    await delay(150)
    const body = (await request.json()) as PartialUserSettings
    return HttpResponse.json(body)
  }),
]
