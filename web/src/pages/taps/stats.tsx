import { useTranslation } from 'react-i18next'
import { useParams, Link } from 'react-router-dom'
import { Activity, Users, TrendingUp, Database } from 'lucide-react'
import { useTap, useTapStats, useTapAuditLog } from '@/features/taps'
import { useAuthStore } from '@/features/auth'
import { usePagination } from '@/hooks'
import { StatsCard } from '@/components/dashboard/stats-card'
import { TimeSeriesChart, DataPagination } from '@/components/common'
import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
} from '@/components/ui/card'
import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
} from '@/components/ui/table'
import { Skeleton } from '@/components/ui/skeleton'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ROUTES } from '@/lib/constants'
import { formatRelativeTime } from '@/lib/date'
import { TapAuditLogEntry } from '@zako-ac/zako3-data'

export const TapStatsPage = () => {
    const { t, i18n } = useTranslation()
    const { tapId } = useParams<{ tapId: string }>()
    const { user } = useAuthStore()
    const { pagination, setPage, getPaginationInfo } = usePagination({
        initialPerPage: 10,
    })

    const { data: tap, isLoading: isTapLoading } = useTap(tapId)
    const { data: stats, isLoading: isStatsLoading } = useTapStats(tapId)
    const { data: auditLogData, isLoading: isAuditLoading } = useTapAuditLog(
        tapId,
        {
            page: pagination.page,
            perPage: pagination.perPage,
        }
    )

    const auditLogs = auditLogData?.data ?? []
    const paginationInfo = getPaginationInfo(auditLogData?.meta)

    // Check if the current user is the owner of the tap
    const isOwner = user && tap && tap.ownerId === user.id

    if (isTapLoading || isStatsLoading) {
        return (
            <div className="space-y-6">
                <div>
                    <Skeleton className="mb-2 h-8 w-48" />
                    <Skeleton className="h-4 w-96" />
                </div>
                <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
                    {Array.from({ length: 4 }).map((_, i) => (
                        <Card key={i}>
                            <CardHeader className="pb-2">
                                <Skeleton className="h-4 w-24" />
                            </CardHeader>
                            <CardContent>
                                <Skeleton className="h-8 w-16" />
                            </CardContent>
                        </Card>
                    ))}
                </div>
            </div>
        )
    }

    if (!tap || !stats) {
        return (
            <div className="py-12 text-center">
                <p className="text-muted-foreground">Tap not found</p>
            </div>
        )
    }

    const formatPercentage = (value: number) => `${value.toFixed(1)}%`

    return (
        <div className="space-y-6">
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-semibold">{tap.name}</h1>
                    <p className="text-muted-foreground">{t('taps.stats.subtitle')}</p>
                </div>
                {isOwner && (
                    <Button asChild variant="outline">
                        <Link to={ROUTES.TAP_SETTINGS(tapId!)}>Settings</Link>
                    </Button>
                )}
            </div>

            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
                <StatsCard
                    title={t('taps.stats.activeNow')}
                    value={stats.currentlyActive.toLocaleString()}
                    icon={<Activity className="h-4 w-4" />}
                />
                <StatsCard
                    title={t('dashboard.stats.totalUses')}
                    value={stats.totalUses.toLocaleString()}
                    icon={<TrendingUp className="h-4 w-4" />}
                />
                <StatsCard
                    title={t('taps.stats.cacheHits')}
                    value={stats.cacheHits.toLocaleString()}
                    icon={<Database className="h-4 w-4" />}
                />
                <StatsCard
                    title={t('taps.stats.uniqueUsers')}
                    value={stats.uniqueUsers.toLocaleString()}
                    icon={<Users className="h-4 w-4" />}
                />
            </div>

            <div className="grid gap-6 lg:grid-cols-2">
                <TimeSeriesChart
                    title={t('taps.stats.useRate')}
                    data={stats.useRateHistory}
                    valueFormatter={(value) => value.toLocaleString()}
                />
                <TimeSeriesChart
                    title={t('taps.stats.cacheHitRate')}
                    data={stats.cacheHitRateHistory}
                    valueFormatter={formatPercentage}
                />
            </div>

            <Card>
                <CardHeader>
                    <CardTitle>{t('taps.stats.auditLog')}</CardTitle>
                    <CardDescription>
                        Recent activity and events for this tap
                    </CardDescription>
                </CardHeader>
                <CardContent>
                    {isAuditLoading ? (
                        <div className="space-y-2">
                            {Array.from({ length: 5 }).map((_, i) => (
                                <Skeleton key={i} className="h-12 w-full" />
                            ))}
                        </div>
                    ) : auditLogs.length === 0 ? (
                        <p className="text-muted-foreground py-8 text-center">
                            {t('taps.stats.noAuditLogs')}
                        </p>
                    ) : (
                        <>
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHead>Event</TableHead>
                                        <TableHead>User</TableHead>
                                        <TableHead>Time</TableHead>
                                        <TableHead>Details</TableHead>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {auditLogs.map((log: TapAuditLogEntry) => (
                                        <TableRow key={log.id}>
                                            <TableCell>
                                                <Badge variant="outline">{log.action}</Badge>
                                            </TableCell>
                                            <TableCell className="font-mono text-xs">
                                                {log.actorId || 'System'}
                                            </TableCell>
                                            <TableCell className="text-muted-foreground">
                                                {formatRelativeTime(log.createdAt, i18n.language)}
                                            </TableCell>
                                            <TableCell className="text-muted-foreground text-sm">
                                                {log.details || '-'}
                                            </TableCell>
                                        </TableRow>
                                    ))}
                                </TableBody>
                            </Table>
                            {auditLogData?.meta && paginationInfo.totalPages > 1 && (
                                <div className="mt-4">
                                    <DataPagination
                                        meta={auditLogData.meta}
                                        onPageChange={setPage}
                                        onPerPageChange={() => { }}
                                    />
                                </div>
                            )}
                        </>
                    )}
                </CardContent>
            </Card>
        </div>
    )
}
