import { useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { UserSettingsCard } from '@/features/settings/user-settings-card'
import { useAdminUserSettings, useSaveAdminUserSettings } from '@/features/settings/use-user-settings'
import { emptyPartial } from '@/features/settings/types'
import { useTaps } from '@/features/taps'

export const AdminUserSettingsUserPage = () => {
    const { userId } = useParams<{ userId: string }>()
    const { t } = useTranslation()

    const { data: settings, isLoading } = useAdminUserSettings(userId ?? '')
    const { data: tapsData } = useTaps()
    const { mutateAsync: save, isPending: isSaving } = useSaveAdminUserSettings(userId ?? '')

    const taps = tapsData?.data ?? []

    const handleSave = async (updatedSettings: any) => {
        try {
            await save(updatedSettings)
            toast.success(t('settings.saveSuccess'))
        } catch {
            toast.error(t('settings.saveError'))
        }
    }

    if (!userId) return null

    return (
        <div className="py-6">
            {isLoading ? (
                <div className="text-center text-muted-foreground">
                    {t('common.loading')}
                </div>
            ) : (
                <UserSettingsCard
                    initialValue={(settings as any) ?? emptyPartial}
                    taps={taps}
                    onSave={handleSave}
                    isSaving={isSaving}
                />
            )}
        </div>
    )
}
