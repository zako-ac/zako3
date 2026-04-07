import { Badge } from '@/components/ui/badge'
import { Crown, CheckCircle } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { TapOccupation } from '@zako-ac/zako3-data'
import { cn } from '@/lib/utils'

interface OccupationBadgeProps {
    occupation: TapOccupation
    className?: string
}

const occupationVariants: Record<
    TapOccupation,
    { className: string }
> = {
    official: {
        className: 'bg-primary text-primary-foreground',
    },
    verified: {
        className: 'bg-success text-success-foreground',
    },
    base: { className: 'bg-secondary text-secondary-foreground' },
}

export const OccupationBadge = ({ occupation, className }: OccupationBadgeProps) => {
    const { t } = useTranslation()

    const occupationTranslations = {
        official: t('taps.occupations.official'),
        verified: t('taps.occupations.verified'),
        base: t('taps.occupations.base'),
    }

    const variant = occupationVariants[occupation]

    return (
        <Badge
            className={cn('shrink-0 gap-1 truncate', variant.className, className)}
        >
            {occupation === 'official' ? (
                <Crown className="h-3 w-3 flex-shrink-0" />
            ) : occupation === 'verified' ? (
                <CheckCircle className="h-3 w-3 flex-shrink-0" />
            ) : null}
            <span className="truncate">{occupationTranslations[occupation]}</span>
        </Badge>
    )
}
