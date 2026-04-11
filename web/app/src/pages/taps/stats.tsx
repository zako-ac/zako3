import { useTranslation } from 'react-i18next'
import { useParams } from 'react-router-dom'
import { useState } from 'react'
import { Activity, Users, TrendingUp, Database, Copy, Check } from 'lucide-react'
import { toast } from 'sonner'
import { useTap, useTapStats, useTapAuditLog, tapKeys } from '@/features/taps'
import { useStatsSSE } from '@/features/stats'
import { OccupationBadge } from '@/components/tap'
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
import { formatRelativeTime } from '@/lib/date'
import { TapAuditLogEntry } from '@zako-ac/zako3-data'
import { UserBadge } from '@/components/tap/user-badge'

export const TapStatsPage = () => {
    const { t, i18n } = useTranslation()
    const [copied, setCopied] = useState(false)
    const { tapId } = useParams<{ tapId: string }>()

    const handleCopyId = () => {
        if (tapId) {
            navigator.clipboard.writeText(tapId)
            setCopied(true)
            toast.success(t('common.copied'))
            setTimeout(() => setCopied(false), 2000)
        }
    }

    const { pagination, setPage, getPaginationInfo } = usePagination({
        initialPerPage: 10,
    })

    useStatsSSE(tapId ? [tapKeys.stats(tapId)] : [])
    const { data: tap, isLoading: isTapLoading } = useTap(tapId)
    const { data: stats, isLoading: isStatsLoading, isFetching: isStatsFetching, refetch: refetchStats } = useTapStats(tapId)
    const { data: auditLogData, isLoading: isAuditLoading } = useTapAuditLog(
        tapId,
        {
            page: pagination.page,
            perPage: pagination.perPage,
        }
    )

    const auditLogs = auditLogData?.data ?? []
    const paginationInfo = getPaginationInfo(auditLogData?.meta)

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
                <p className="text-muted-foreground">{t('taps.notFound')}</p>
            </div>
        )
    }

    const formatPercentage = (value: number) => `${value.toFixed(1)}%`

    return (
        <div className="space-y-6">
            <div className="flex items-center justify-between">
                <div>
                    <div className="flex items-center gap-2">
                        <h1 className="text-2xl font-semibold">{tap.name}</h1>
                        <OccupationBadge occupation={tap.occupation} />
                    </div>
                    <div className="flex items-center gap-2">
                        <p className="text-muted-foreground font-mono text-sm">
                            {tap.id}
                        </p>
                        <Button
                            variant="ghost"
                            size="icon"
                            className="h-5 w-5 hover:bg-transparent"
                            onClick={handleCopyId}
                        >
                            {copied ? (
                                <Check className="text-success h-3.5 w-3.5" />
                            ) : (
                                <Copy className="text-muted-foreground h-3.5 w-3.5" />
                            )}
                        </Button>
                    </div>
                </div>
            </div>

            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
                <StatsCard
                    title={t('taps.stats.activeNow')}
                    value={stats.currentlyActive.toLocaleString()}
                    icon={<Activity className="h-4 w-4" />}
                    onRefresh={refetchStats}
                    isRefreshing={isStatsFetching}
                />
                <StatsCard
                    title={t('dashboard.stats.totalUses')}
                    value={stats.totalUses.toLocaleString()}
                    icon={<TrendingUp className="h-4 w-4" />}
                    onRefresh={refetchStats}
                    isRefreshing={isStatsFetching}
                />
                <StatsCard
                    title={t('taps.stats.cacheHits')}
                    value={stats.cacheHits.toLocaleString()}
                    icon={<Database className="h-4 w-4" />}
                    onRefresh={refetchStats}
                    isRefreshing={isStatsFetching}
                />
                <StatsCard
                    title={t('taps.stats.uniqueUsers')}
                    value={stats.uniqueUsers.toLocaleString()}
                    icon={<Users className="h-4 w-4" />}
                    onRefresh={refetchStats}
                    isRefreshing={isStatsFetching}
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
                                                <Badge variant="outline">{log.actionType}</Badge>
                                            </TableCell>
                                            <TableCell>
                                                <UserBadge actor={log.actor} />
                                            </TableCell>
                                            <TableCell className="text-muted-foreground">
                                                {formatRelativeTime(log.createdAt, i18n.language)}
                                            </TableCell>
                                            <TableCell className="text-muted-foreground text-sm">
                                                {log.details || '-'}
                                            </TableCell>
                                        </TableRow>
                                    )
                                    )}
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
