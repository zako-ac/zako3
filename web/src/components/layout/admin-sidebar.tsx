import zakoLogo from '@/assets/zakopsa.png'
import { Link, useLocation } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import {
    LayoutDashboard,
    Users,
    Compass,
    Bell,
    Settings,
    LogOut,
    ChevronRight,
    ExternalLink,
    ArrowLeft,
    ShieldCheck,
} from 'lucide-react'
import { ROUTES } from '@/lib/constants'
import { useLogout, useAuthStore } from '@/features/auth'
import { useVerificationRequests } from '@/features/admin'
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
    SidebarMenuSub,
    SidebarMenuSubButton,
    SidebarMenuSubItem,
    useSidebar,
} from '@/components/ui/sidebar'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
    Collapsible,
    CollapsibleContent,
    CollapsibleTrigger,
} from '@/components/ui/collapsible'
import clsx from 'clsx'

interface NavItem {
    title: string
    url: string
    icon: React.ComponentType<{ className?: string }>
    external?: boolean
    items?: { title: string; url: string }[]
    badge?: number
}

export const AdminSidebar = () => {
    const { t } = useTranslation()
    const location = useLocation()
    const { user } = useAuthStore()
    const { mutate: logout } = useLogout()
    const { state } = useSidebar()

    // Get pending verifications count
    const { data: verificationsData } = useVerificationRequests({
        status: 'pending',
        perPage: 1,
    })
    const pendingCount = verificationsData?.meta?.total ?? 0

    const adminNavItems: NavItem[] = [
        {
            title: t('admin.overview'),
            url: ROUTES.ADMIN,
            icon: LayoutDashboard,
        },
        {
            title: t('nav.users'),
            url: ROUTES.ADMIN_USERS,
            icon: Users,
        },
        {
            title: t('nav.taps'),
            url: ROUTES.ADMIN_TAPS,
            icon: Compass,
        },
        {
            title: t('admin.verifications.sidebarTitle'),
            url: ROUTES.ADMIN_VERIFICATIONS,
            icon: ShieldCheck,
            badge: pendingCount,
        },
        {
            title: t('nav.notifications'),
            url: ROUTES.ADMIN_NOTIFICATIONS,
            icon: Bell,
        },
        {
            title: 'Grafana',
            url: 'https://grafana.zako.ac',
            icon: ExternalLink,
            external: true,
        },
    ]

    const userNavItems: NavItem[] = [
        {
            title: t('nav.dashboard'),
            url: ROUTES.DASHBOARD,
            icon: LayoutDashboard,
        },
        {
            title: t('nav.taps'),
            url: ROUTES.TAPS,
            icon: Compass,
            items: [
                { title: t('nav.explore'), url: ROUTES.TAPS },
                { title: t('nav.myTaps'), url: ROUTES.TAPS_MINE },
            ],
        },
    ]

    const isActive = (url: string) => location.pathname === url
    const isGroupActive = (item: NavItem) =>
        isActive(item.url) || item.items?.some((sub) => isActive(sub.url))

    const renderNavItem = (item: NavItem) => {
        if (item.external) {
            return (
                <SidebarMenuItem key={item.title}>
                    <SidebarMenuButton asChild tooltip={item.title}>
                        <a href={item.url} target="_blank" rel="noopener noreferrer">
                            <item.icon className="h-4 w-4" />
                            <span>{item.title}</span>
                            <ExternalLink className="ml-auto h-3 w-3" />
                        </a>
                    </SidebarMenuButton>
                </SidebarMenuItem>
            )
        }

        if (item.items) {
            return (
                <Collapsible
                    key={item.title}
                    asChild
                    defaultOpen={isGroupActive(item)}
                    className="group/collapsible"
                >
                    <SidebarMenuItem>
                        <CollapsibleTrigger asChild>
                            <SidebarMenuButton
                                tooltip={item.title}
                                isActive={isGroupActive(item)}
                            >
                                <item.icon className="h-4 w-4" />
                                <span>{item.title}</span>
                                <ChevronRight className="ml-auto h-4 w-4 transition-transform duration-200 group-data-[state=open]/collapsible:rotate-90" />
                            </SidebarMenuButton>
                        </CollapsibleTrigger>
                        <CollapsibleContent>
                            <SidebarMenuSub>
                                {item.items.map((subItem) => (
                                    <SidebarMenuSubItem key={subItem.url}>
                                        <SidebarMenuSubButton
                                            asChild
                                            isActive={isActive(subItem.url)}
                                        >
                                            <Link to={subItem.url}>{subItem.title}</Link>
                                        </SidebarMenuSubButton>
                                    </SidebarMenuSubItem>
                                ))}
                            </SidebarMenuSub>
                        </CollapsibleContent>
                    </SidebarMenuItem>
                </Collapsible>
            )
        }

        return (
            <SidebarMenuItem key={item.title}>
                <SidebarMenuButton
                    asChild
                    tooltip={item.title}
                    isActive={isActive(item.url)}
                >
                    <Link to={item.url}>
                        <item.icon className="h-4 w-4" />
                        <span>{item.title}</span>
                        {item.badge !== undefined && item.badge > 0 && (
                            <Badge variant="secondary" className="ml-auto h-5 px-1.5 text-xs">
                                {item.badge}
                            </Badge>
                        )}
                    </Link>
                </SidebarMenuButton>
            </SidebarMenuItem>
        )
    }

    return (
        <Sidebar collapsible="icon">
            <SidebarHeader className="border-sidebar-border border-b">
                <SidebarMenu>
                    <SidebarMenuItem>
                        <Link
                            to={ROUTES.ADMIN}
                            className="flex items-center gap-2 px-2 py-1"
                        >
                            <img
                                className={clsx(
                                    'rounded-lg',
                                    state === 'expanded' ? 'h-8 w-8' : 'h-4 w-4'
                                )}
                                alt="ZAKO"
                                src={zakoLogo}
                            />
                            {state === 'expanded' && (
                                <div className="flex flex-col">
                                    <span className="text-lg font-semibold">ZAKO</span>
                                    <span className="text-muted-foreground text-xs">Admin</span>
                                </div>
                            )}
                        </Link>
                    </SidebarMenuItem>
                </SidebarMenu>
            </SidebarHeader>

            <SidebarContent>
                <SidebarGroup>
                    <SidebarGroupLabel>{t('nav.admin')}</SidebarGroupLabel>
                    <SidebarGroupContent>
                        <SidebarMenu>{adminNavItems.map(renderNavItem)}</SidebarMenu>
                    </SidebarGroupContent>
                </SidebarGroup>

                <SidebarGroup>
                    <SidebarGroupLabel>{t('nav.dashboard')}</SidebarGroupLabel>
                    <SidebarGroupContent>
                        <SidebarMenu>{userNavItems.map(renderNavItem)}</SidebarMenu>
                    </SidebarGroupContent>
                </SidebarGroup>
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
                                            {user?.username?.slice(0, 2).toUpperCase() || 'A'}
                                        </AvatarFallback>
                                    </Avatar>
                                    {state === 'expanded' && (
                                        <div className="grid flex-1 text-left text-sm leading-tight">
                                            <span className="truncate font-semibold">
                                                {user?.username}
                                            </span>
                                            <span className="text-muted-foreground truncate text-xs">
                                                Admin
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
                                        to={ROUTES.DASHBOARD}
                                        className="flex items-center gap-2"
                                    >
                                        <ArrowLeft className="h-4 w-4" />
                                        {t('nav.backToApp')}
                                    </Link>
                                </DropdownMenuItem>
                                <DropdownMenuItem asChild>
                                    <Link
                                        to={ROUTES.SETTINGS}
                                        className="flex items-center gap-2"
                                    >
                                        <Settings className="h-4 w-4" />
                                        {t('nav.settings')}
                                    </Link>
                                </DropdownMenuItem>
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
