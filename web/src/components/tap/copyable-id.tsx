import { useState } from 'react'
import { Copy, Check } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

interface CopyableIdProps {
    id: string
    className?: string
}

export const CopyableId = ({ id, className }: CopyableIdProps) => {
    const { t } = useTranslation()
    const [copied, setCopied] = useState(false)

    const handleCopy = (e: React.MouseEvent) => {
        e.stopPropagation()
        navigator.clipboard.writeText(id)
        setCopied(true)
        toast.success(t('common.copied'))
        setTimeout(() => setCopied(false), 2000)
    }

    return (
        <div className={cn('flex items-center gap-1.5', className)}>
            <p className="text-muted-foreground mt-0.5 font-mono text-xs">
                {id}
            </p>
            <Button
                variant="ghost"
                size="icon"
                className="h-4 w-4 shrink-0 hover:bg-transparent"
                onClick={handleCopy}
            >
                {copied ? (
                    <Check className="text-success h-3 w-3" />
                ) : (
                    <Copy className="h-3 w-3" />
                )}
            </Button>
        </div>
    )
}
