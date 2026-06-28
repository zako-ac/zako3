import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useAuthStore } from '@/features/auth'
import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
} from '@/components/ui/card'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Separator } from '@/components/ui/separator'
import { ThemeToggle } from '@/components/layout/theme-toggle'
import { LanguageToggle } from '@/components/layout/language-toggle'
import { Label } from '@/components/ui/label'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { AlertTriangle, Key } from 'lucide-react'
import { useTaps } from '@/features/taps'
import {
    useApiKeys,
    useCreateApiKey,
    useRevokeApiKey,
    CreateApiKeyDialog,
    ApiKeyItem,
} from '@/features/api-keys'
import type { CreateUserApiKeyInput } from '@zako-ac/zako3-data'
import { UserSettingsCard, usePartialUserSettings, useSavePartialUserSettings, emptyPartial } from '@/features/settings'
import type { PartialUserSettings } from '@/features/settings'
import { LoadingSkeleton } from '@/components/common'
import { toast } from 'sonner'

export const SettingsPage = () => {
    const { t } = useTranslation()
    const { user } = useAuthStore()

    const { data: tapsData } = useTaps()
    const { data: settingsData, isLoading: isLoadingSettings } = usePartialUserSettings()
    const { mutateAsync: saveSettings } = useSavePartialUserSettings()

    const [createKeyDialogOpen, setCreateKeyDialogOpen] = useState(false)
    const { data: apiKeys, isLoading: isLoadingKeys } = useApiKeys()
    const { mutateAsync: createApiKey, isPending: isCreatingKey } = useCreateApiKey()
    const { mutateAsync: revokeApiKey, isPending: isRevokingKey } = useRevokeApiKey()

    if (!user) return null

    const handleCreateApiKey = async (data: CreateUserApiKeyInput) => {
        try {
            const result = await createApiKey(data)
            toast.success(t('settings.apiKeys.created'))
            return result
        } catch (error) {
            toast.error(error instanceof Error ? error.message : t('settings.apiKeys.createError'))
            throw error
        }
    }

    const handleRevokeApiKey = async (keyId: string) => {
        try {
            await revokeApiKey(keyId)
            toast.success(t('settings.apiKeys.revokedToast'))
        } catch (error) {
            toast.error(error instanceof Error ? error.message : t('settings.apiKeys.revokeError'))
        }
    }

    const handleSaveSettings = async (settings: PartialUserSettings) => {
        try {
            await saveSettings(settings)
            toast.success(t('settings.saveSuccess'))
        } catch {
            toast.error(t('settings.saveError'))
        }
    }

    return (
        <div className="mx-auto max-w-4xl space-y-6">
            <div>
                <h1 className="text-2xl font-semibold">{t('settings.title')}</h1>
                <p className="text-muted-foreground">{t('settings.subtitle')}</p>
            </div>

            <Card>
                <CardHeader>
                    <CardTitle>{t('settings.profile')}</CardTitle>
                    <CardDescription>{t('settings.profileSubtitle')}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    <div className="flex items-center gap-4">
                        <Avatar className="h-16 w-16">
                            <AvatarImage src={user.avatar} alt={user.username} />
                            <AvatarFallback>
                                {user.username.slice(0, 2).toUpperCase()}
                            </AvatarFallback>
                        </Avatar>
                        <div>
                            <p className="font-semibold">{user.username}</p>
                            <p className="text-muted-foreground text-sm">{user.email}</p>
                            <p className="text-muted-foreground font-mono text-xs">
                                {user.id}
                            </p>
                        </div>
                    </div>
                    <Separator />
                    <div className="space-y-2">
                        <Label>{t('common.role')}</Label>
                        <p className="text-sm">{user.isAdmin ? 'Administrator' : 'User'}</p>
                    </div>
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <CardTitle>{t('settings.appearance')}</CardTitle>
                    <CardDescription>{t('settings.appearanceSubtitle')}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    <div className="flex items-center justify-between">
                        <div className="space-y-0.5">
                            <Label>{t('settings.darkMode')}</Label>
                            <p className="text-muted-foreground text-sm">
                                {t('settings.darkModeSubtitle')}
                            </p>
                        </div>
                        <ThemeToggle />
                    </div>
                    <Separator />
                    <div className="flex items-center justify-between">
                        <div className="space-y-0.5">
                            <Label>{t('settings.language')}</Label>
                            <p className="text-muted-foreground text-sm">
                                {t('settings.languageSubtitle')}
                            </p>
                        </div>
                        <LanguageToggle />
                    </div>
                </CardContent>
            </Card>

            {isLoadingSettings ? (
                <LoadingSkeleton count={1} variant="card" />
            ) : (
                <UserSettingsCard
                    initialValue={settingsData ?? emptyPartial}
                    taps={tapsData?.data ?? []}
                    onSave={handleSaveSettings}
                />
            )}

            <Card>
                <CardHeader>
                    <CardTitle>{t('settings.notifications')}</CardTitle>
                    <CardDescription>
                        {t('settings.notificationsSubtitle')}
                    </CardDescription>
                </CardHeader>
                <CardContent>
                    <p className="text-muted-foreground text-sm">
                        {t('settings.notificationsCheckBackLater')}
                    </p>
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <div className="flex items-center justify-between">
                        <div>
                            <CardTitle className="flex items-center gap-2">
                                <Key className="h-5 w-5" />
                                {t('settings.apiKeys.title')}
                            </CardTitle>
                            <CardDescription>
                                {t('settings.apiKeys.subtitle')}
                            </CardDescription>
                        </div>
                        <Button
                            type="button"
                            variant="outline"
                            onClick={() => setCreateKeyDialogOpen(true)}
                            disabled={isLoadingKeys}
                        >
                            {t('settings.apiKeys.createKey')}
                        </Button>
                    </div>
                </CardHeader>
                <CardContent>
                    {isLoadingKeys ? (
                        <div className="space-y-3">
                            <Skeleton className="h-20 w-full" />
                            <Skeleton className="h-20 w-full" />
                        </div>
                    ) : apiKeys && apiKeys.length > 0 ? (
                        <div className="space-y-3">
                            {apiKeys.map((key) => (
                                <ApiKeyItem
                                    key={key.id}
                                    apiKey={key}
                                    onRevoke={handleRevokeApiKey}
                                    isRevoking={isRevokingKey}
                                />
                            ))}
                        </div>
                    ) : (
                        <div className="text-muted-foreground rounded-lg border border-dashed p-8 text-center">
                            <Key className="mx-auto mb-2 h-8 w-8 opacity-50" />
                            <p className="mb-1 text-sm font-medium">
                                {t('settings.apiKeys.noKeys')}
                            </p>
                            <p className="text-xs">{t('settings.apiKeys.noKeysDescription')}</p>
                        </div>
                    )}
                </CardContent>
            </Card>

            <Card className="border-destructive/50">
                <CardHeader>
                    <CardTitle className="text-destructive flex items-center gap-2">
                        <AlertTriangle className="h-5 w-5" />
                        {t('settings.account')}
                    </CardTitle>
                    <CardDescription>{t('settings.accountSubtitle')}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    <div>
                        <Label>{t('settings.deleteAccount')}</Label>
                        <p className="text-muted-foreground mt-1 mb-3 text-sm">
                            {t('settings.deleteAccountWarning')}
                        </p>
                        <Button variant="destructive" disabled>
                            {t('settings.deleteAccount')}
                        </Button>
                    </div>
                </CardContent>
            </Card>

            <CreateApiKeyDialog
                open={createKeyDialogOpen}
                onOpenChange={setCreateKeyDialogOpen}
                onSubmit={handleCreateApiKey}
                isLoading={isCreatingKey}
            />
        </div>
    )
}
