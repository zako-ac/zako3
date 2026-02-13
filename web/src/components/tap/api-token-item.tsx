import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Copy, RotateCw, Trash2, Check } from 'lucide-react'
import { formatDistanceToNow } from 'date-fns'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ConfirmDialog } from '@/components/common'
import { useClipboard } from '@/hooks/use-clipboard'
import type { TapApiToken } from '@zako-ac/zako3-data'

interface ApiTokenItemProps {
    token: TapApiToken
    onRegenerate: (tokenId: string) => void
    onDelete: (tokenId: string) => void
    onNewToken?: (fullToken: string) => void
    isRegenerating?: boolean
    isDeleting?: boolean
}

export const ApiTokenItem = ({
    token,
    onRegenerate,
    onDelete,
    isRegenerating,
    isDeleting,
}: ApiTokenItemProps) => {
    const { t } = useTranslation()
    const { copied, copy } = useClipboard()
    const [deleteDialogOpen, setDeleteDialogOpen] = useState(false)
    const [regenerateDialogOpen, setRegenerateDialogOpen] = useState(false)

    const handleCopy = async () => {
        await copy(token.token)
        toast.success(t('taps.settings.tokenCopied'))
    }

    const handleRegenerate = () => {
        onRegenerate(token.id)
        setRegenerateDialogOpen(false)
    }

    const handleDelete = () => {
        onDelete(token.id)
        setDeleteDialogOpen(false)
    }

    const formatDate = (date: string) => {
        return formatDistanceToNow(new Date(date), { addSuffix: true })
    }

    const isExpired = token.expiresAt && new Date(token.expiresAt) < new Date()

    return (
        <>
            <div className="border-border rounded-lg border p-4">
                <div className="mb-3 flex items-start justify-between">
                    <div>
                        <h4 className="font-medium">{token.label}</h4>
                        <p className="text-muted-foreground text-xs">
                            {t('taps.settings.tokenCreatedAt')}: {formatDate(token.createdAt)}
                        </p>
                    </div>
                    <div className="flex gap-2">
                        <Button
                            size="sm"
                            variant="outline"
                            onClick={() => setRegenerateDialogOpen(true)}
                            disabled={isRegenerating || isDeleting}
                        >
                            <RotateCw className="h-4 w-4" />
                        </Button>
                        <Button
                            size="sm"
                            variant="outline"
                            onClick={() => setDeleteDialogOpen(true)}
                            disabled={isRegenerating || isDeleting}
                        >
                            <Trash2 className="h-4 w-4" />
                        </Button>
                    </div>
                </div>

                <div className="mb-2 flex gap-2">
                    <Input
                        value={token.token}
                        readOnly
                        className="font-mono text-xs"
                        type="text"
                    />
                    <Button
                        size="sm"
                        variant="outline"
                        onClick={handleCopy}
                        className="shrink-0"
                    >
                        {copied ? (
                            <Check className="h-4 w-4" />
                        ) : (
                            <Copy className="h-4 w-4" />
                        )}
                    </Button>
                </div>

                <div className="text-muted-foreground flex flex-wrap gap-3 text-xs">
                    <span>
                        {t('taps.settings.tokenLastUsed')}:{' '}
                        {token.lastUsedAt
                            ? formatDate(token.lastUsedAt)
                            : t('taps.settings.tokenNeverUsed')}
                    </span>
                    {token.expiresAt && (
                        <span className={isExpired ? 'text-destructive' : ''}>
                            {isExpired
                                ? t('taps.settings.tokenExpired')
                                : `${t('taps.settings.tokenExpires')}: ${formatDate(token.expiresAt)}`}
                        </span>
                    )}
                    {!token.expiresAt && (
                        <span>{t('taps.settings.tokenNeverExpires')}</span>
                    )}
                </div>
            </div>

            <ConfirmDialog
                open={regenerateDialogOpen}
                onOpenChange={setRegenerateDialogOpen}
                title={t('taps.settings.regenerateToken')}
                description={t('taps.settings.regenerateConfirm')}
                confirmLabel={t('taps.settings.regenerate')}
                onConfirm={handleRegenerate}
                isLoading={isRegenerating}
                variant="default"
            />

            <ConfirmDialog
                open={deleteDialogOpen}
                onOpenChange={setDeleteDialogOpen}
                title={t('taps.settings.deleteToken')}
                description={t('taps.settings.deleteTokenConfirm')}
                confirmLabel={t('common.delete')}
                onConfirm={handleDelete}
                isLoading={isDeleting}
                variant="destructive"
            />
        </>
    )
}
