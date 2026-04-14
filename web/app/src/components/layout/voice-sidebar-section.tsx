import { Link, useLocation } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { Server, Settings, Volume2 } from 'lucide-react'
import { useMyGuilds } from '@/features/guild'
import { ROUTES } from '@/lib/constants'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import {
    SidebarGroup,
    SidebarGroupContent,
    SidebarGroupLabel,
    SidebarMenu,
    SidebarMenuButton,
    SidebarMenuAction,
    SidebarMenuItem,
    SidebarMenuSub,
    SidebarMenuSubButton,
    SidebarMenuSubItem,
} from '@/components/ui/sidebar'

export const VoiceSidebarSection = () => {
    const { t } = useTranslation()
    const location = useLocation()
    const { data: guilds } = useMyGuilds()

    if (!guilds || guilds.length === 0) {
        return null
    }

    return (
        <SidebarGroup>
            <SidebarGroupLabel>Guilds</SidebarGroupLabel>
            <SidebarGroupContent>
                <SidebarMenu>
                    {guilds.map((guild) => {
                        const voiceUrl = guild.activeChannelId
                            ? ROUTES.VOICE_CHANNEL(guild.guildId, guild.activeChannelId)
                            : null
                        const isVoiceActive =
                            voiceUrl && location.pathname === voiceUrl

                        return (
                            <SidebarMenuItem key={guild.guildId}>
                                <Link
                                    to={ROUTES.GUILD_SETTINGS(guild.guildId)}
                                    title={t('guilds.settings.action')}
                                >
                                    <SidebarMenuButton tooltip={guild.guildName || guild.guildId}>
                                        <Avatar className="h-5 w-5 rounded-sm">
                                            <AvatarImage src={guild.guildIconUrl ?? undefined} />
                                            <AvatarFallback className="rounded-sm">
                                                <Server className="h-3 w-3" />
                                            </AvatarFallback>
                                        </Avatar>
                                        <span>{guild.guildName || `Server ...${guild.guildId.slice(-6)}`}</span>
                                    </SidebarMenuButton>
                                    <SidebarMenuAction asChild>
                                        <div className="flex items-center justify-center">
                                            <Settings className="h-3.5 w-3.5 opacity-60" />
                                        </div>
                                    </SidebarMenuAction>
                                </Link>


                                {guild.activeChannelId && guild.activeChannelName && (
                                    <SidebarMenuSub>
                                        <SidebarMenuSubItem>
                                            <SidebarMenuSubButton
                                                asChild
                                                isActive={isVoiceActive || false}
                                            >
                                                <Link to={voiceUrl || '#'} className="flex items-center gap-2">
                                                    <Volume2 className="h-3.5 w-3.5" />
                                                    <span>{guild.activeChannelName}</span>
                                                </Link>
                                            </SidebarMenuSubButton>
                                        </SidebarMenuSubItem>
                                    </SidebarMenuSub>
                                )}
                            </SidebarMenuItem>
                        )
                    })}
                </SidebarMenu>
            </SidebarGroupContent>
        </SidebarGroup>
    )
}
