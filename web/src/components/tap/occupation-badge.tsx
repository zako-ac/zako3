import { Badge } from '@/components/ui/badge'
import { Crown, CheckCircle } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { TapOccupation } from '@zako-ac/zako3-data'

interface OccupationBadgeProps {
  occupation: TapOccupation
}

export const OccupationBadge = ({ occupation }: OccupationBadgeProps) => {
  const { t } = useTranslation()

  if (occupation === 'base') return null

  return (
    <Badge
      variant={occupation === 'official' ? 'default' : 'secondary'}
      className="gap-1"
    >
      {occupation === 'official' ? (
        <Crown className="h-3 w-3" />
      ) : (
        <CheckCircle className="h-3 w-3" />
      )}
      {t(`taps.occupations.${occupation}`)}
    </Badge>
  )
}
