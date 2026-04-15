import { http, HttpResponse, delay } from 'msw'
import { API_BASE } from './base'
import type { GuildSummaryDto } from '@zako-ac/zako3-data'

const mockGuilds: GuildSummaryDto[] = [
  {
    guildId: 'guild_1',
    guildName: 'Development Server',
    guildIconUrl: 'https://cdn.discordapp.com/icons/1234567890/a_1234567890abcdef1234567890abcdef.png',
    canManage: true,
  },
  {
    guildId: 'guild_2',
    guildName: 'Test Community',
    guildIconUrl: 'https://cdn.discordapp.com/icons/0987654321/b_0987654321fedcba0987654321fedcba.png',
    canManage: true,
  },
  {
    guildId: 'guild_3',
    guildName: 'Music Hub',
    canManage: false,
  },
]

export const guildHandlers = [
  http.get(`${API_BASE}/guilds/me`, async () => {
    await delay(150)
    return HttpResponse.json(mockGuilds)
  }),

  http.get(`${API_BASE}/admin/users/:userId/guilds`, async () => {
    await delay(150)
    return HttpResponse.json(mockGuilds)
  }),
]
