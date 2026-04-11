import { useParams, useLocation, useNavigate, Outlet } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { emptyPartial, foldPartial } from '@/features/settings'
import type { PartialUserSettings } from '@/features/settings'
import type { TapWithAccess } from '@zako-ac/zako3-data'
import {
    useGuildUserSettings,
    useSaveGuildUserSettings,
    useDeleteGuildUserSettings,
    useGuildSettings,
    useSaveGuildSettings,
    useGlobalSettings,
} from '@/features/settings'
import { useMyGuilds } from '@/features/guild'
import { useTaps } from '@/features/taps'
import { ROUTES } from '@/lib/constants'

export interface GuildSettingsOutletContext {
    taps: TapWithAccess[]
    guildUserPartial: PartialUserSettings | undefined
    guildPartial: PartialUserSettings | undefined
    guildUpstream: PartialUserSettings
    globalUpstream: PartialUserSettings
    canManage: boolean
    handleSaveGuildUser: (s: PartialUserSettings) => Promise<void>
    handleResetGuildUser: () => Promise<void>
    handleSaveGuild: (s: PartialUserSettings) => Promise<void>
    savingGuildUser: boolean
    deletingGuildUser: boolean
    savingGuild: boolean
}

export const GuildSettingsPage = () => {
    const { guildId } = useParams<{ guildId: string }>()
    const location = useLocation()
    const navigate = useNavigate()
    const { t } = useTranslation()

    const { data: guilds } = useMyGuilds()
    const guild = guilds?.find((g) => g.guildId === guildId)
    const canManage = guild?.canManage ?? false

    const { data: tapsData } = useTaps()
    const taps = tapsData?.data ?? []

    // GuildUser scope
    const { data: guildUserPartial } = useGuildUserSettings(guildId ?? '')
    const { mutateAsync: saveGuildUser, isPending: savingGuildUser } = useSaveGuildUserSettings(guildId ?? '')
    const { mutateAsync: deleteGuildUser, isPending: deletingGuildUser } = useDeleteGuildUserSettings(guildId ?? '')

    // Guild scope
    const { data: guildPartial } = useGuildSettings(guildId ?? '')
    const { mutateAsync: saveGuild, isPending: savingGuild } = useSaveGuildSettings(guildId ?? '')

    // Global scope (for upstream warnings)
    const { data: globalPartial } = useGlobalSettings()

    const handleSaveGuildUser = async (settings: PartialUserSettings) => {
        try {
            await saveGuildUser(settings)
            toast.success(t('settings.saveSuccess'))
        } catch {
            toast.error(t('settings.saveError'))
        }
    }

    const handleResetGuildUser = async () => {
        try {
            await deleteGuildUser()
            toast.success(t('guilds.settings.resetSuccess'))
        } catch {
            toast.error(t('settings.saveError'))
        }
    }

    const handleSaveGuild = async (settings: PartialUserSettings) => {
        try {
            await saveGuild(settings)
            toast.success(t('settings.saveSuccess'))
        } catch {
            toast.error(t('settings.saveError'))
        }
    }

    // Upstream for "My Settings" tab = guild + global folded
    const guildUpstream = foldPartial(
        guildPartial ?? emptyPartial,
        globalPartial ?? emptyPartial,
    )
    // Upstream for "Guild Settings" tab = global only
    const globalUpstream = globalPartial ?? emptyPartial

    // Determine active tab from URL
    const tabValue = location.pathname.endsWith('/guild') ? 'guild-settings' : 'my-settings'

    const handleTabChange = (value: string) => {
        if (value === 'guild-settings') {
            navigate(ROUTES.GUILD_SETTINGS_GUILD(guildId ?? ''))
        } else {
            navigate(ROUTES.GUILD_SETTINGS_ME(guildId ?? ''))
        }
    }

    if (!guildId) return null

    const outletContext: GuildSettingsOutletContext = {
        taps,
        guildUserPartial,
        guildPartial,
        guildUpstream,
        globalUpstream,
        canManage,
        handleSaveGuildUser,
        handleResetGuildUser,
        handleSaveGuild,
        savingGuildUser,
        deletingGuildUser,
        savingGuild,
    }

    return (
        <div className="space-y-6">
            <div>
                <h1 className="text-2xl font-semibold">{t('guilds.settings.title')}</h1>
                <p className="text-muted-foreground text-sm">
                    {guild?.guildName ?? guildId}
                </p>
            </div>

            <Tabs value={tabValue} onValueChange={handleTabChange}>
                <TabsList>
                    <TabsTrigger value="my-settings">{t('guilds.settings.mySettings')}</TabsTrigger>
                    <TabsTrigger value="guild-settings" disabled={!canManage}>{t('guilds.settings.guildSettings')}</TabsTrigger>
                </TabsList>

                <Outlet context={outletContext} />
            </Tabs>
        </div>
    )
}
