import { Music, MessageSquare } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import type { TapRole } from '@zako-ac/zako3-data'
import { cn } from '@/lib/utils'

interface TapRolesBadgeProps {
    roles: TapRole[]
    className?: string
}

const roleIcons: Record<TapRole, React.ReactNode> = {
    music: <Music className="h-3 w-3" />,
    tts: <MessageSquare className="h-3 w-3" />,
}

export const TapRolesBadge = ({ roles, className }: TapRolesBadgeProps) => {
    const { t } = useTranslation()

    return (
        <div className={cn('flex flex-wrap items-center gap-2', className)}>
            {roles.map((role) => (
                <Badge key={role} variant="outline" className="gap-1">
                    {roleIcons[role]}
                    {t(`taps.roleLabels.${role}`)}
                </Badge>
            ))}
        </div>
    )
}
