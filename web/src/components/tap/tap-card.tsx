import { useTranslation } from 'react-i18next'
import {
    Flag,
    Music,
    MessageSquare,
    Settings,
    Copy,
    Check,
} from 'lucide-react'
import { motion } from 'framer-motion'
import { useState } from 'react'
import { toast } from 'sonner'
import { Card, CardContent, CardFooter, CardHeader } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
    Tooltip,
    TooltipContent,
    TooltipProvider,
    TooltipTrigger,
} from '@/components/ui/tooltip'
import { formatRelativeTime } from '@/lib/date'
import { cn } from '@/lib/utils'
import type { TapWithAccess, TapOccupation, TapRole } from '@zako-ac/zako3-data'
import { PermissionBadge } from './permission-badge'
import { UserBadge } from './user-badge'

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
    const [copied, setCopied] = useState(false)
    const occupation = occupationVariants[tap.occupation]

    const handleCopyId = (e: React.MouseEvent) => {
        e.stopPropagation()
        navigator.clipboard.writeText(tap.id)
        setCopied(true)
        toast.success(t('common.copied'))
        setTimeout(() => setCopied(false), 2000)
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
                    'group hover:border-primary/50 hover:shadow-primary/10 cursor-pointer transition-all hover:shadow-lg',
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
                            <div className="flex items-center gap-1.5">
                                <p className="text-muted-foreground mt-0.5 font-mono text-xs">
                                    {tap.id}
                                </p>
                                <Button
                                    variant="ghost"
                                    size="icon"
                                    className="h-4 w-4 shrink-0 hover:bg-transparent"
                                    onClick={handleCopyId}
                                >
                                    {copied ? (
                                        <Check className="text-success h-3 w-3" />
                                    ) : (
                                        <Copy className="h-3 w-3" />
                                    )}
                                </Button>
                            </div>
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
                    <p className="text-muted-foreground line-clamp-2 min-h-10 text-sm">
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
