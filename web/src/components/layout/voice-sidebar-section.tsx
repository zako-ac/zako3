import { Link, useLocation } from 'react-router-dom'
import { Volume2 } from 'lucide-react'
import { usePlaybackState } from '@/features/playback'
import { ROUTES } from '@/lib/constants'
import {
    SidebarGroup,
    SidebarGroupContent,
    SidebarGroupLabel,
    SidebarMenu,
    SidebarMenuButton,
    SidebarMenuItem,
    SidebarMenuSub,
    SidebarMenuSubButton,
    SidebarMenuSubItem,
} from '@/components/ui/sidebar'

export const VoiceSidebarSection = () => {
    const location = useLocation()
    const { data: states } = usePlaybackState()

    if (!states || states.length === 0) {
        return null
    }

    // Group channels by guildId
    const byGuild = states.reduce<Record<string, typeof states>>(
        (acc, state) => {
            if (!acc[state.guildId]) acc[state.guildId] = []
            acc[state.guildId].push(state)
            return acc
        },
        {}
    )

    return (
        <SidebarGroup>
            <SidebarGroupLabel>
                <Volume2 className="mr-1.5 h-3.5 w-3.5" />
                Voice
            </SidebarGroupLabel>
            <SidebarGroupContent>
                <SidebarMenu>
                    {Object.entries(byGuild).map(([guildId, channels]) => (
                        <SidebarMenuItem key={guildId}>
                            <SidebarMenuButton tooltip={channels[0].guildName || guildId}>
                                <Volume2 className="h-4 w-4" />
                                <span>{channels[0].guildName || `Server ...${guildId.slice(-6)}`}</span>
                            </SidebarMenuButton>
                            <SidebarMenuSub>
                                {channels.map((state) => {
                                    const url = ROUTES.VOICE_CHANNEL(
                                        state.guildId,
                                        state.channelId
                                    )
                                    const trackCount = Object.values(
                                        state.queues as Record<string, unknown[]>
                                    ).reduce((n, q) => n + q.length, 0)
                                    const isActive =
                                        location.pathname === url
                                    return (
                                        <SidebarMenuSubItem
                                            key={state.channelId}
                                        >
                                            <SidebarMenuSubButton
                                                asChild
                                                isActive={isActive}
                                            >
                                                <Link to={url}>
                                                    <span>
                                                        #{state.channelName || `...${state.channelId.slice(-6)}`}
                                                    </span>
                                                    {trackCount > 0 && (
                                                        <span className="ml-auto text-xs text-muted-foreground">
                                                            {trackCount}
                                                        </span>
                                                    )}
                                                </Link>
                                            </SidebarMenuSubButton>
                                        </SidebarMenuSubItem>
                                    )
                                })}
                            </SidebarMenuSub>
                        </SidebarMenuItem>
                    ))}
                </SidebarMenu>
            </SidebarGroupContent>
        </SidebarGroup>
    )
}
