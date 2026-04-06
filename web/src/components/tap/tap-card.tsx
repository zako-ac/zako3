import { useTranslation } from 'react-i18next'
import {
    Flag,
    Settings,
} from 'lucide-react'
import { motion } from 'framer-motion'
import { Card, CardContent, CardFooter, CardHeader } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import {
    Tooltip,
    TooltipContent,
    TooltipProvider,
    TooltipTrigger,
} from '@/components/ui/tooltip'
import { formatRelativeTime } from '@/lib/date'
import { cn } from '@/lib/utils'
import type { TapWithAccess } from '@zako-ac/zako3-data'
import { PermissionBadge } from './permission-badge'
import { UserBadge } from './user-badge'
import { OccupationBadge } from './occupation-badge'
import { CopyableId } from './copyable-id'
import { TapRolesBadge } from './tap-roles-badge'
import { SetAsMyVoice } from './set-as-my-voice'

interface TapCardProps {
    tap: TapWithAccess
    onReport: (tapId: string) => void
    onClick?: (tapId: string) => void
    onSettingsClick?: (tapId: string) => void
}

export const TapCard = ({
    tap,
    onReport,
    onClick,
    onSettingsClick,
}: TapCardProps) => {
    const { t, i18n } = useTranslation()

    const handleReport = (e: React.MouseEvent) => {
        e.stopPropagation()
        onReport(tap.id)
    }

    const handleSettingsClick = (e: React.MouseEvent) => {
        e.stopPropagation()
        onSettingsClick?.(tap.id)
    }

    return (
        <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.2 }}
        >
            <Card
                className={cn(
                    'group transition-all',
                    onClick ? 'hover:border-primary/50 hover:shadow-primary/10 cursor-pointer hover:shadow-lg' : 'cursor-default'
                )}
                onClick={() => onClick?.(tap.id)}
            >
                <CardHeader className="pb-2">
                    <div className="flex items-start justify-between gap-2">
                        <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-2">
                                <h3 className="truncate font-semibold">{tap.name}</h3>
                                <OccupationBadge occupation={tap.occupation} />
                            </div>
                            <CopyableId id={tap.id} />
                        </div>
                        <SetAsMyVoice
                            tapId={tap.id}
                            hasTtsRole={tap.roles.includes('tts')}
                            hasAccess={tap.hasAccess}
                        />
                        {onSettingsClick && (
                            <TooltipProvider>
                                <Tooltip>
                                    <TooltipTrigger asChild>
                                        <Button
                                            variant="ghost"
                                            size="icon-sm"
                                            className="shrink-0"
                                            onClick={handleSettingsClick}
                                        >
                                            <Settings className="h-4 w-4" />
                                        </Button>
                                    </TooltipTrigger>
                                    <TooltipContent>{t('taps.settings.title')}</TooltipContent>
                                </Tooltip>
                            </TooltipProvider>
                        )}
                    </div>
                </CardHeader>

                <CardContent className="pb-2">
                    <p className="text-muted-foreground line-clamp-2 min-h-10 text-sm">
                        {tap.description || 'No description'}
                    </p>

                    <div className="mt-3 flex flex-wrap items-center gap-2">
                        <TapRolesBadge roles={tap.roles} />

                        <PermissionBadge hasAccess={tap.hasAccess} permission={tap.permission} />
                    </div>
                </CardContent>

                <CardFooter className="flex items-center justify-between border-t pt-2">
                    <UserBadge user={tap.owner} />

                    <div className="text-muted-foreground flex items-center gap-3 text-xs">
                        <span>{tap.totalUses.toLocaleString()} uses</span>
                        <span>{formatRelativeTime(tap.createdAt, i18n.language)}</span>
                        <Button
                            variant="ghost"
                            size="icon-sm"
                            className="opacity-0 transition-opacity group-hover:opacity-100"
                            onClick={handleReport}
                        >
                            <Flag className="h-4 w-4" />
                        </Button>
                    </div>
                </CardFooter>
            </Card>
        </motion.div>
    )
}
