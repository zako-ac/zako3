import { useQuery } from '@tanstack/react-query'
import { guildApi } from './api'

export const guildKeys = {
    all: ['guilds'] as const,
    myGuilds: () => [...guildKeys.all, 'my'] as const,
}

export const useMyGuilds = () => {
    return useQuery({
        queryKey: guildKeys.myGuilds(),
        queryFn: () => guildApi.getMyGuilds(),
        staleTime: 1000 * 60, // 1 minute
    })
}
