import { useTranslation } from 'react-i18next'
import { useOutletContext } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { Trash2 } from 'lucide-react'
import { UserSettingsCard } from '@/features/settings'
import { emptyPartial } from '@/features/settings'
import type { PartialUserSettings } from '@/features/settings'
import { LoadingSkeleton } from '@/components/common'
import type { GuildSettingsOutletContext } from './GuildSettingsPage'

export const GuildMySettingsPage = () => {
    const { t } = useTranslation()
    const {
        taps,
        guildUserPartial,
        guildUpstream,
        handleSaveGuildUser,
        savingGuildUser,
        handleResetGuildUser,
        deletingGuildUser,
    } = useOutletContext<GuildSettingsOutletContext>()

    const handleSave = async (settings: PartialUserSettings) => {
        await handleSaveGuildUser(settings)
    }

    // Show loading state while initial data is fetching
    const isLoading = guildUserPartial === undefined

    return (
        <>
            <p className="text-muted-foreground text-sm">
                {t('guilds.settings.mySettingsDescription')}
            </p>
            {isLoading ? (
                <LoadingSkeleton count={1} variant="card" />
            ) : (
                <>
                    <UserSettingsCard
                        initialValue={guildUserPartial ?? emptyPartial}
                        taps={taps}
                        onSave={handleSave}
                        isSaving={savingGuildUser}
                        upstreamSettings={guildUpstream}
                    />
                    <div className="flex justify-end">
                        <Button
                            variant="outline"
                            size="sm"
                            onClick={handleResetGuildUser}
                            disabled={deletingGuildUser}
                        >
                            <Trash2 className="mr-2 h-4 w-4" />
                            {t('guilds.settings.resetToDefault')}
                        </Button>
                    </div>
                </>
            )}
        </>
    )
}
