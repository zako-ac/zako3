import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Trash2 } from 'lucide-react'
import { formatDistanceToNow } from 'date-fns'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { ConfirmDialog } from '@/components/common'
import { CopyableId } from '@/components/tap/copyable-id'
import type { UserApiKey } from '@zako-ac/zako3-data'

interface ApiKeyItemProps {
    apiKey: UserApiKey
    onRevoke: (keyId: string) => void
    isRevoking?: boolean
}

export const ApiKeyItem = ({ apiKey, onRevoke, isRevoking }: ApiKeyItemProps) => {
    const { t } = useTranslation()
    const [revokeDialogOpen, setRevokeDialogOpen] = useState(false)

    const formatDate = (date: string) =>
        formatDistanceToNow(new Date(date), { addSuffix: true })

    const isExpired = apiKey.expiresAt && new Date(apiKey.expiresAt) < new Date()

    const handleRevoke = () => {
        onRevoke(apiKey.id)
        setRevokeDialogOpen(false)
    }

    return (
        <>
            <div className="border-border space-y-3 rounded-lg border p-4">
                <div className="flex items-start justify-between gap-2">
                    <div className="space-y-1">
                        <div className="flex items-center gap-2">
                            <h4 className="font-medium">{apiKey.label}</h4>
                            {apiKey.revoked && (
                                <Badge variant="destructive">
                                    {t('settings.apiKeys.revoked')}
                                </Badge>
                            )}
                        </div>
                        <CopyableId id={apiKey.id} />
                    </div>
                    {!apiKey.revoked && (
                        <Button
                            size="sm"
                            variant="outline"
                            onClick={() => setRevokeDialogOpen(true)}
                            disabled={isRevoking}
                        >
                            <Trash2 className="h-4 w-4" />
                        </Button>
                    )}
                </div>

                <div className="text-muted-foreground flex flex-wrap gap-3 text-xs">
                    <span>
                        {t('settings.apiKeys.createdAt')}: {formatDate(apiKey.createdAt)}
                    </span>
                    <span>
                        {t('settings.apiKeys.lastUsed')}:{' '}
                        {apiKey.lastUsedAt
                            ? formatDate(apiKey.lastUsedAt)
                            : t('settings.apiKeys.neverUsed')}
                    </span>
                    {apiKey.expiresAt ? (
                        <span className={isExpired ? 'text-destructive' : ''}>
                            {isExpired
                                ? t('settings.apiKeys.expired')
                                : `${t('settings.apiKeys.expires')}: ${formatDate(apiKey.expiresAt)}`}
                        </span>
                    ) : (
                        <span>{t('settings.apiKeys.neverExpires')}</span>
                    )}
                </div>
            </div>

            <ConfirmDialog
                open={revokeDialogOpen}
                onOpenChange={setRevokeDialogOpen}
                title={t('settings.apiKeys.revokeKey')}
                description={t('settings.apiKeys.revokeConfirm')}
                confirmLabel={t('settings.apiKeys.revoke')}
                onConfirm={handleRevoke}
                isLoading={isRevoking}
                variant="destructive"
            />
        </>
    )
}
