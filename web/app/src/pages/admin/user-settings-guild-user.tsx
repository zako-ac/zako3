import { useState } from 'react'
import { useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { GuildSelectDialog } from '@/components/common'
import { UserSettingsCard } from '@/features/settings/user-settings-card'
import { useAdminUserGuilds } from '@/features/guild/hooks'
import { useAdminUserGuildSettings, useSaveAdminUserGuildSettings } from '@/features/settings/use-user-settings'
import { emptyPartial } from '@/features/settings/types'
import { useTaps } from '@/features/taps'
import type { GuildSummaryDto } from '@zako-ac/zako3-data'

export const AdminUserSettingsGuildUserPage = () => {
    const { userId } = useParams<{ userId: string }>()
    const { t } = useTranslation()
    const [selectedGuild, setSelectedGuild] = useState<GuildSummaryDto | null>(null)
    const [dialogOpen, setDialogOpen] = useState(false)

    const { data: guilds = [], isLoading: guildsLoading } = useAdminUserGuilds(userId ?? '')
    const { data: settings, isLoading: settingsLoading } = useAdminUserGuildSettings(
        userId ?? '',
        selectedGuild?.guildId ?? ''
    )
    const { data: tapsData } = useTaps()
    const { mutateAsync: save, isPending: isSaving } = useSaveAdminUserGuildSettings(
        userId ?? '',
        selectedGuild?.guildId ?? ''
    )

    const taps = tapsData?.data ?? []

    const handleSelectGuild = (guild: GuildSummaryDto) => {
        setSelectedGuild(guild)
        setDialogOpen(false)
    }

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
        <div className="space-y-6 py-6">
            {/* Guild Selection */}
            <Card>
                <CardHeader>
                    <CardTitle>{t('admin.userSettings.selectGuild')}</CardTitle>
                    <CardDescription>
                        {selectedGuild
                            ? t('admin.userSettings.currentGuild', { guild: selectedGuild.guildName })
                            : t('admin.userSettings.noGuildSelected')}
                    </CardDescription>
                </CardHeader>
                <CardContent>
                    <Button onClick={() => setDialogOpen(true)} disabled={guildsLoading}>
                        {selectedGuild ? t('admin.userSettings.changeGuild') : t('admin.userSettings.selectGuild')}
                    </Button>
                </CardContent>
            </Card>

            {/* Settings */}
            {selectedGuild && (
                <>
                    {settingsLoading ? (
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
                </>
            )}

            {/* Guild Selection Dialog */}
            <GuildSelectDialog
                open={dialogOpen}
                onOpenChange={setDialogOpen}
                onSelect={handleSelectGuild}
                guilds={guilds}
                isLoading={guildsLoading}
            />
        </div>
    )
}
