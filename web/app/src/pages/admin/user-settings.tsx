import { useParams, useLocation, useNavigate, Outlet } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ROUTES } from '@/lib/constants'

export const AdminUserSettingsPage = () => {
    const { userId } = useParams<{ userId: string }>()
    const location = useLocation()
    const navigate = useNavigate()
    const { t } = useTranslation()

    if (!userId) return null

    // Determine active tab from URL
    const tabValue = location.pathname.endsWith('/guild-user') ? 'guild-user' : 'user'

    const handleTabChange = (value: string) => {
        if (value === 'guild-user') {
            navigate(ROUTES.ADMIN_USER_SETTINGS_GUILD_USER(userId))
        } else {
            navigate(ROUTES.ADMIN_USER_SETTINGS_USER(userId))
        }
    }

    return (
        <div className="space-y-6">
            <div>
                <h1 className="text-2xl font-semibold">{t('admin.userSettings.title')}</h1>
                <p className="text-muted-foreground text-sm">
                    {t('admin.userSettings.userId')}: {userId}
                </p>
            </div>

            <Tabs value={tabValue} onValueChange={handleTabChange}>
                <TabsList>
                    <TabsTrigger value="user">{t('admin.userSettings.userScope')}</TabsTrigger>
                    <TabsTrigger value="guild-user">{t('admin.userSettings.guildUserScope')}</TabsTrigger>
                </TabsList>

                <Outlet />
            </Tabs>
        </div>
    )
}
