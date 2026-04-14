import { useTranslation } from 'react-i18next'
import { Compass } from 'lucide-react'
import { TapCard } from './tap-card'
import { EmptyState, LoadingSkeleton } from '@/components/common'
import { useAuthStore } from '@/features/auth'
import type { TapWithAccess } from '@zako-ac/zako3-data'

interface TapListProps {
    taps: TapWithAccess[]
    isLoading?: boolean
    onReport: (tapId: string) => void
    onTapClick?: (tapId: string) => void
    onSettingsClick?: (tapId: string) => void
    emptyMessage?: string
    emptyDescription?: string
}

export const TapList = ({
    taps,
    isLoading,
    onReport,
    onTapClick,
    onSettingsClick,
    emptyMessage,
    emptyDescription,
}: TapListProps) => {
    const { t } = useTranslation()
    const { user } = useAuthStore()

    if (isLoading) {
        return <LoadingSkeleton count={6} variant="card" />
    }

    if (taps.length === 0) {
        return (
            <EmptyState
                icon={<Compass className="h-8 w-8" />}
                title={emptyMessage || t('taps.noTaps')}
                description={emptyDescription}
            />
        )
    }

    return (
        <div className="flex gap-4 flex-wrap">
            {taps.map((tap) => {
                const canClick = !!(onTapClick && (user?.id === tap.owner.id || user?.isAdmin))
                return (
                    <TapCard
                        key={tap.id}
                        tap={tap}
                        onReport={onReport}
                        onClick={canClick ? onTapClick : undefined}
                        onSettingsClick={onSettingsClick}
                    />
                )
            })}
        </div>
    )
}
