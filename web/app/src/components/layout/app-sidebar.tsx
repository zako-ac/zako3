import zakoLogo from '@/assets/zakopsa.png'
import { Link, useLocation, matchPath } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import {
    LayoutDashboard,
    Settings,
    LogOut,
    Shield,
    Plus,
    Box,
    Globe,
    Activity,
    Key,
    ShieldCheck,
} from 'lucide-react'

import { ROUTES } from '@/lib/constants'
import { useLogout, useAuthStore } from '@/features/auth'
import { useTap } from '@/features/taps'
import {
    Sidebar,
    SidebarContent,
    SidebarFooter,
    SidebarGroup,
    SidebarGroupContent,
    SidebarGroupLabel,
    SidebarHeader,
    SidebarMenu,
    SidebarMenuButton,
    SidebarMenuItem,
    useSidebar,
} from '@/components/ui/sidebar'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import clsx from 'clsx'
import { VoiceSidebarSection } from './voice-sidebar-section'

export const AppSidebar = () => {
    const { t } = useTranslation()
    const location = useLocation()
    const { user } = useAuthStore()
    const { mutate: logout } = useLogout()
    const { state } = useSidebar()

    const match = matchPath('/taps/:tapId/*', location.pathname) || matchPath('/taps/:tapId', location.pathname)
    const activeTapId = match?.params.tapId
    const isValidTapId = activeTapId && !['create', 'mine'].includes(activeTapId)
    const { data: tapData } = useTap(isValidTapId ? activeTapId : undefined)

    const navSections: {
        title: string
        items: {
            title: string
            url: string
            icon: React.ComponentType<{ className?: string }>
            isPrimary?: boolean
            isSubItem?: boolean
        }[]
    }[] = [
            {
                title: t('nav.dashboard'),
                items: [
                    {
                        title: t('nav.dashboard'),
                        url: ROUTES.DASHBOARD,
                        icon: LayoutDashboard,
                    },
                ],
            },
            {
                title: t('nav.taps'),
                items: [
                    {
                        title: t('nav.createTap'),
                        url: ROUTES.TAPS_CREATE,
                        icon: Plus,
                        isPrimary: true,
                    },
                    {
                        title: t('nav.explore'),
                        url: ROUTES.TAPS,
                        icon: Globe,
                    },
                    {
                        title: t('nav.myTaps'),
                        url: ROUTES.TAPS_MINE,
                        icon: Box,
                    },
                ],
            },
            ...(isValidTapId && tapData
                ? [
                    {
                        title: t('admin.taps.manageTap'),
                        items: [
                            {
                                title: t('taps.stats.title'),
                                url: ROUTES.TAP_STATS(activeTapId),
                                icon: Activity,
                            },
                            {
                                title: t('nav.settings'),
                                url: ROUTES.TAP_SETTINGS(activeTapId),
                                icon: Settings,
                            },
                             {
                                 title: t('taps.settings.apiAccess'),
                                 url: ROUTES.TAP_API_KEYS(activeTapId),
                                 icon: Key,
                             },
                             {
                                 title: t('nav.verification'),
                                 url: ROUTES.TAP_VERIFICATION(activeTapId),
                                 icon: ShieldCheck,
                             },
                         ],
                     },
                 ]

                : []),
        ]

    const isActive = (url: string) => {
        if (location.pathname === url) return true
        return false
    }

    return (
        <Sidebar collapsible="icon">
            <SidebarHeader className="border-sidebar-border border-b">
                <SidebarMenu>
                    <SidebarMenuItem>
                        <Link
                            to={ROUTES.DASHBOARD}
                            className="flex items-center gap-2 px-2 py-1"
                        >
                            <img
                                className={clsx('rounded-lg', state === 'expanded' ? 'h-8 w-8' : 'h-4 w-4')}
                                alt="ZAKO"
                                src={zakoLogo}
                            />
                            {state === 'expanded' && (
                                <span className="text-lg font-semibold">ZAKO</span>
                            )}
                        </Link>
                    </SidebarMenuItem>
                </SidebarMenu>
            </SidebarHeader>

            <SidebarContent>
                {user && <VoiceSidebarSection />}
            {navSections.map((section) => (
                    <SidebarGroup key={section.title}>
                        <SidebarGroupLabel>{section.title}</SidebarGroupLabel>
                        <SidebarGroupContent>
                            <SidebarMenu>
                                {section.items.map((item) => (
                                    <SidebarMenuItem key={item.title}>
                                        <SidebarMenuButton
                                            asChild
                                            tooltip={item.title}
                                            isActive={isActive(item.url) || (item.isSubItem && location.pathname.startsWith(`/taps/${activeTapId}`))}
                                            className={clsx(
                                                item.isPrimary && 'bg-primary text-primary-foreground hover:bg-primary/90 hover:text-primary-foreground',
                                                item.isSubItem && 'pl-8'
                                            )}
                                        >
                                            <Link to={item.url}>
                                                <item.icon className="h-4 w-4" />
                                                <span>{item.title}</span>
                                            </Link>
                                        </SidebarMenuButton>
                                    </SidebarMenuItem>
                                ))}
                            </SidebarMenu>
                        </SidebarGroupContent>
                    </SidebarGroup>
                ))}
            </SidebarContent>

            <SidebarFooter className="border-sidebar-border border-t">
                <SidebarMenu>
                    <SidebarMenuItem>
                        <DropdownMenu>
                            <DropdownMenuTrigger asChild>
                                <SidebarMenuButton
                                    size="lg"
                                    className="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
                                >
                                    <Avatar className="h-8 w-8 rounded-lg">
                                        <AvatarImage src={user?.avatar} alt={user?.username} />
                                        <AvatarFallback className="rounded-lg">
                                            {user?.username?.slice(0, 2).toUpperCase() || 'U'}
                                        </AvatarFallback>
                                    </Avatar>
                                    {state === 'expanded' && (
                                        <div className="grid flex-1 text-left text-sm leading-tight">
                                            <span className="truncate font-semibold">
                                                {user?.username}
                                            </span>
                                            <span className="text-muted-foreground truncate text-xs">
                                                {user?.email}
                                            </span>
                                        </div>
                                    )}
                                </SidebarMenuButton>
                            </DropdownMenuTrigger>
                            <DropdownMenuContent
                                className="w-[--radix-dropdown-menu-trigger-width] min-w-56 rounded-lg"
                                side="top"
                                align="start"
                                sideOffset={4}
                            >
                                <DropdownMenuItem asChild>
                                    <Link
                                        to={ROUTES.SETTINGS}
                                        className="flex items-center gap-2"
                                    >
                                        <Settings className="h-4 w-4" />
                                        {t('nav.settings')}
                                    </Link>
                                </DropdownMenuItem>
                                {user?.isAdmin && (
                                    <>
                                        <DropdownMenuSeparator />
                                        <DropdownMenuItem asChild>
                                            <Link
                                                to={ROUTES.ADMIN}
                                                className="flex items-center gap-2"
                                            >
                                                <Shield className="h-4 w-4" />
                                                {t('nav.adminPanel')}
                                            </Link>
                                        </DropdownMenuItem>
                                    </>
                                )}
                                <DropdownMenuSeparator />
                                <DropdownMenuItem
                                    onClick={() => logout()}
                                    className="text-destructive focus:text-destructive"
                                >
                                    <LogOut className="h-4 w-4" />
                                    {t('nav.logout')}
                                </DropdownMenuItem>
                            </DropdownMenuContent>
                        </DropdownMenu>
                    </SidebarMenuItem>
                </SidebarMenu>
            </SidebarFooter>
        </Sidebar>
    )
}
