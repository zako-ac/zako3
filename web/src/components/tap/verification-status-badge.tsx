import { Badge } from '@/components/ui/badge'
import { CheckCircle2, XCircle, Clock } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { VerificationStatus } from '@zako-ac/zako3-data'
import { cn } from '@/lib/utils'

interface VerificationStatusBadgeProps {
    status: VerificationStatus
    className?: string
}

const statusConfigs: Record<
    VerificationStatus,
    { icon: typeof CheckCircle2; className: string }
> = {
    approved: {
        icon: CheckCircle2,
        className: 'bg-success/15 text-success border-success/20',
    },
    rejected: {
        icon: XCircle,
        className: 'bg-destructive/15 text-destructive border-destructive/20',
    },
    pending: {
        icon: Clock,
        className: 'bg-warning/15 text-warning border-warning/20',
    },
}

export const VerificationStatusBadge = ({
    status,
    className,
}: VerificationStatusBadgeProps) => {
    const { t } = useTranslation()
    const config = statusConfigs[status]
    const Icon = config.icon

    return (
        <Badge
            variant="outline"
            className={cn('gap-1.5 px-2 py-0.5 font-medium', config.className, className)}
        >
            <Icon className="h-3.5 w-3.5" />
            {t(`taps.verification.status.${status}`)}
        </Badge>
    )
}
