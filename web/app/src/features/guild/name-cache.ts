import { create } from 'zustand'

interface NameCacheState {
    guilds: Record<string, string>
    channels: Record<string, string>
    ingestGuilds(data: { guildId: string; guildName: string; activeChannelId?: string | null; activeChannelName?: string | null }[]): void
    ingestPlayback(data: { guildId: string; guildName: string; channelId: string; channelName: string }[]): void
}

export const useNameCache = create<NameCacheState>((set) => ({
    guilds: {},
    channels: {},
    ingestGuilds(data) {
        set((s) => {
            const guilds = { ...s.guilds }
            const channels = { ...s.channels }
            for (const g of data) {
                guilds[g.guildId] = g.guildName
                if (g.activeChannelId && g.activeChannelName) {
                    channels[g.activeChannelId] = g.activeChannelName
                }
            }
            return { guilds, channels }
        })
    },
    ingestPlayback(data) {
        set((s) => {
            const guilds = { ...s.guilds }
            const channels = { ...s.channels }
            for (const st of data) {
                guilds[st.guildId] = st.guildName
                channels[st.channelId] = st.channelName
            }
            return { guilds, channels }
        })
    },
}))

export const useGuildName = (guildId: string) =>
    useNameCache((s) => s.guilds[guildId])

export const useChannelName = (channelId: string) =>
    useNameCache((s) => s.channels[channelId])
