import { useTranslation } from 'react-i18next'
import {
    Flag,
    Check,
    Music,
    MessageSquare,
    Copy,
    Settings,
} from 'lucide-react'
import { motion } from 'framer-motion'
import { Card, CardContent, CardFooter, CardHeader } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import {
    Tooltip,
    TooltipContent,
    TooltipProvider,
    TooltipTrigger,
} from '@/components/ui/tooltip'
import { formatRelativeTime } from '@/lib/date'
import { useClipboard } from '@/hooks'
import { cn } from '@/lib/utils'
import type { TapWithAccess, TapOccupation, TapRole } from '@zako-ac/zako3-data'
import { PermissionBadge } from './permission-badge'

const occupationVariants: Record<
    TapOccupation,
    { label: string; className: string }
> = {
    official: {
        label: 'Official',
        className: 'bg-primary text-primary-foreground',
    },
    verified: {
        label: 'Verified',
        className: 'bg-success text-success-foreground',
    },
    base: { label: 'Base', className: 'bg-secondary text-secondary-foreground' },
}

const roleIcons: Record<TapRole, React.ReactNode> = {
    music: <Music className="h-3 w-3" />,
    tts: <MessageSquare className="h-3 w-3" />,
}

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
    const { copied, copy } = useClipboard()

    const occupation = occupationVariants[tap.occupation]

    const handleCopyUserId = (e: React.MouseEvent) => {
        e.stopPropagation()
        copy(tap.owner.id)
    }

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
                    'group hover:border-primary/50 hover:shadow-primary/10 cursor-pointer transition-all hover:-translate-y-1 hover:shadow-lg',
                    onClick && 'cursor-pointer'
                )}
                onClick={() => onClick?.(tap.id)}
            >
                <CardHeader className="pb-2">
                    <div className="flex items-start justify-between gap-2">
                        <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-2">
                                <h3 className="truncate font-semibold">{tap.name}</h3>
                                <Badge className={cn('shrink-0', occupation.className)}>
                                    {t(`taps.occupations.${tap.occupation}`)}
                                </Badge>
                            </div>
                            <p className="text-muted-foreground mt-0.5 font-mono text-xs">
                                {tap.id}
                            </p>
                        </div>
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
                    <p className="text-muted-foreground line-clamp-2 min-h-[2.5rem] text-sm">
                        {tap.description || 'No description'}
                    </p>

                    <div className="mt-3 flex flex-wrap items-center gap-2">
                        {tap.roles.map((role) => (
                            <Badge key={role} variant="outline" className="gap-1">
                                {roleIcons[role]}
                                {t(`taps.roleLabels.${role}`)}
                            </Badge>
                        ))}

                        <PermissionBadge hasAccess={tap.hasAccess} permission={tap.permission} />
                    </div>
                </CardContent>

                <CardFooter className="flex items-center justify-between border-t pt-2">
                    <TooltipProvider>
                        <Tooltip>
                            <TooltipTrigger asChild>
                                <button
                                    className="text-muted-foreground hover:text-foreground flex items-center gap-2 text-sm transition-colors"
                                    onClick={handleCopyUserId}
                                >
                                    <Avatar className="h-5 w-5">
                                        <AvatarImage
                                            src={tap.owner.avatar}
                                            alt={tap.owner.username}
                                        />
                                        <AvatarFallback className="text-[10px]">
                                            {tap.owner.username.slice(0, 2).toUpperCase()}
                                        </AvatarFallback>
                                    </Avatar>
                                    <span className="max-w-[100px] truncate">
                                        {tap.owner.username}
                                    </span>
                                    {copied ? (
                                        <Check className="text-success h-3 w-3" />
                                    ) : (
                                        <Copy className="h-3 w-3 opacity-0 transition-opacity group-hover:opacity-100" />
                                    )}
                                </button>
                            </TooltipTrigger>
                            <TooltipContent>
                                {copied ? t('common.copied') : t('common.copyToClipboard')}
                            </TooltipContent>
                        </Tooltip>
                    </TooltipProvider>

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
