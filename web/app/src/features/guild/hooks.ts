import { useEffect } from 'react'
import { useQuery } from '@tanstack/react-query'
import { guildApi } from './api'
import { useNameCache } from './name-cache'

export const guildKeys = {
    all: ['guilds'] as const,
    myGuilds: () => [...guildKeys.all, 'my'] as const,
}

export const useMyGuilds = () => {
    const ingestGuilds = useNameCache((s) => s.ingestGuilds)
    const query = useQuery({
        queryKey: guildKeys.myGuilds(),
        queryFn: () => guildApi.getMyGuilds(),
        staleTime: 1000 * 60,
    })
    useEffect(() => {
        if (query.data) ingestGuilds(query.data)
    }, [query.data, ingestGuilds])
    return query
}
