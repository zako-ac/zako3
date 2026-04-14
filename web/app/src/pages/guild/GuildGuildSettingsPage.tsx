import { useTranslation } from 'react-i18next'
import { useOutletContext } from 'react-router-dom'
import { UserSettingsCard } from '@/features/settings'
import { emptyPartial } from '@/features/settings'
import type { PartialUserSettings } from '@/features/settings'
import { LoadingSkeleton } from '@/components/common'
import type { GuildSettingsOutletContext } from './GuildSettingsPage'

export const GuildGuildSettingsPage = () => {
    const { t } = useTranslation()

    const {
        taps,
        guildPartial,
        globalUpstream,
        canManage,
        handleSaveGuild,
        savingGuild,
    } = useOutletContext<GuildSettingsOutletContext>()

    const handleSave = async (settings: PartialUserSettings) => {
        await handleSaveGuild(settings)
    }

    if (!canManage) {
        return (
            <p className="text-muted-foreground text-sm mb-4">
                {t('guilds.settings.guildSettingsNoPermission')}
            </p>
        )
    }

    // Show loading state while initial data is fetching
    const isLoading = guildPartial === undefined

    return (
        <>
            <p className="text-muted-foreground text-sm mb-4">
                {canManage
                    ? t('guilds.settings.guildSettingsDescription')
                    : t('guilds.settings.guildSettingsNoPermission')}
            </p>
            {isLoading ? (
                <LoadingSkeleton count={1} variant="card" />
            ) : (
                <UserSettingsCard
                    initialValue={guildPartial ?? emptyPartial}
                    taps={taps}
                    onSave={handleSave}
                    isSaving={savingGuild || !canManage}
                    showImportant={canManage}
                    upstreamSettings={globalUpstream}
                    filterOwnerOnly={true}
                />
            )}
        </>
    )
}
