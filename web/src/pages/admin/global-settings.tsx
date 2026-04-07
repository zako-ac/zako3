import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { UserSettingsCard } from '@/features/settings'
import { useGlobalSettings, useSaveGlobalSettings } from '@/features/settings'
import { emptyPartial } from '@/features/settings'
import type { PartialUserSettings } from '@/features/settings'
import { useTaps } from '@/features/taps'

export const AdminGlobalSettingsPage = () => {
    const { t } = useTranslation()

    const { data: tapsData } = useTaps()
    const taps = tapsData?.data ?? []

    const { data: globalPartial } = useGlobalSettings()
    const { mutateAsync: saveGlobal, isPending: saving } = useSaveGlobalSettings()

    const handleSave = async (settings: PartialUserSettings) => {
        try {
            await saveGlobal(settings)
            toast.success(t('settings.saveSuccess'))
        } catch {
            toast.error(t('settings.saveError'))
        }
    }

    return (
        <div className="space-y-6">
            <div>
                <h1 className="text-2xl font-semibold">{t('admin.globalSettings.title')}</h1>
                <p className="text-muted-foreground text-sm">
                    {t('admin.globalSettings.description')}
                </p>
            </div>

            <UserSettingsCard
                initialValue={globalPartial ?? emptyPartial}
                taps={taps}
                onSave={handleSave}
                isSaving={saving}
                showImportant={true}
            />
        </div>
    )
}
