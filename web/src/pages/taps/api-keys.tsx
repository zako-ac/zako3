import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useParams } from 'react-router-dom'
import { toast } from 'sonner'
import { Key } from 'lucide-react'
import type { CreateTapApiTokenInput } from '@zako-ac/zako3-data'
import {
    useTapApiTokens,
    useCreateTapApiToken,
    useRegenerateTapApiToken,
    useDeleteTapApiToken,
} from '@/features/taps'
import { Button } from '@/components/ui/button'
import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
} from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'
import { ApiTokenItem } from '@/components/tap/api-token-item'
import { CreateApiTokenDialog } from '@/components/tap/create-api-token-dialog'

export const TapApiKeysPage = () => {
    const { t } = useTranslation()
    const { tapId } = useParams<{ tapId: string }>()
    const [createTokenDialogOpen, setCreateTokenDialogOpen] = useState(false)

    // API Token hooks
    const { data: apiTokens, isLoading: isLoadingTokens } = useTapApiTokens(tapId)
    const { mutateAsync: createToken, isPending: isCreatingToken } = useCreateTapApiToken(tapId!)
    const { mutateAsync: regenerateToken, isPending: isRegeneratingToken } = useRegenerateTapApiToken(tapId!)
    const { mutateAsync: deleteToken, isPending: isDeletingToken } = useDeleteTapApiToken(tapId!)

    const handleCreateToken = async (data: CreateTapApiTokenInput) => {
        try {
            const result = await createToken(data)
            toast.success(t('taps.settings.tokenCreated'))
            return result
        } catch (error) {
            toast.error(
                error instanceof Error ? error.message : 'Failed to create token'
            )
            throw error
        }
    }

    const handleRegenerateToken = async (tokenId: string) => {
        try {
            const result = await regenerateToken(tokenId)
            toast.success(t('taps.settings.tokenRegenerated'))
            return result
        } catch (error) {
            toast.error(
                error instanceof Error ? error.message : 'Failed to regenerate token'
            )
            throw error
        }
    }

    const handleDeleteToken = async (tokenId: string) => {
        try {
            await deleteToken(tokenId)
            toast.success(t('taps.settings.tokenDeleted'))
        } catch (error) {
            toast.error(
                error instanceof Error ? error.message : 'Failed to delete token'
            )
        }
    }

    return (
        <div className="mx-auto max-w-2xl space-y-6">
            <div>
                <h1 className="text-2xl font-semibold">{t('taps.settings.apiAccess')}</h1>
                <p className="text-muted-foreground">{t('taps.settings.apiAccessDescription')}</p>
            </div>

            <Card>
                <CardHeader>
                    <div className="flex items-center justify-between">
                        <div>
                            <CardTitle className="flex items-center gap-2">
                                <Key className="h-5 w-5" />
                                {t('taps.settings.apiAccess')}
                            </CardTitle>
                            <CardDescription>
                                {t('taps.settings.apiAccessDescription')}
                            </CardDescription>
                        </div>
                        <Button
                            type="button"
                            variant="outline"
                            onClick={() => setCreateTokenDialogOpen(true)}
                            disabled={isLoadingTokens}
                        >
                            {t('taps.settings.createToken')}
                        </Button>
                    </div>
                </CardHeader>
                <CardContent>
                    {isLoadingTokens ? (
                        <div className="space-y-3">
                            <Skeleton className="h-24 w-full" />
                            <Skeleton className="h-24 w-full" />
                        </div>
                    ) : apiTokens && apiTokens.length > 0 ? (
                        <div className="space-y-3">
                            {apiTokens.map((token) => (
                                <ApiTokenItem
                                    key={token.id}
                                    token={token}
                                    onRegenerate={handleRegenerateToken}
                                    onDelete={handleDeleteToken}
                                    isRegenerating={isRegeneratingToken}
                                    isDeleting={isDeletingToken}
                                />
                            ))}
                        </div>
                    ) : (
                        <div className="text-muted-foreground rounded-lg border border-dashed p-8 text-center">
                            <Key className="mx-auto mb-2 h-8 w-8 opacity-50" />
                            <p className="mb-1 text-sm font-medium">
                                {t('taps.settings.noTokens')}
                            </p>
                            <p className="text-xs">{t('taps.settings.noTokensDescription')}</p>
                        </div>
                    )}
                </CardContent>
            </Card>

            <CreateApiTokenDialog
                open={createTokenDialogOpen}
                onOpenChange={setCreateTokenDialogOpen}
                onSubmit={handleCreateToken}
                isLoading={isCreatingToken}
            />
        </div>
    )
}
