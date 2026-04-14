import { apiCall } from '@/lib/api-helpers'
import { apiClient } from '@/lib/api-client'
import type { GuildSummaryDto } from '@zako-ac/zako3-data'

export const guildApi = {
    getMyGuilds: (): Promise<GuildSummaryDto[]> =>
        apiCall(apiClient.get<GuildSummaryDto[]>('/guilds/me')),

    getAdminUserGuilds: (userId: string): Promise<GuildSummaryDto[]> =>
        apiCall(apiClient.get<GuildSummaryDto[]>(`/admin/users/${encodeURIComponent(userId)}/guilds`)),
}
