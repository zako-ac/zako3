import { useState } from 'react'
import { useParams, Link, useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { ArrowLeft, Trash2, Users, Activity, MousePointer2, Database } from 'lucide-react'
import { useTap, useTapStats, useDeleteTap, tapKeys } from '@/features/taps'
import { useStatsSSE } from '@/features/stats'
import { useUpdateTapOccupation } from '@/features/admin/hooks'
import { PermissionBadge, TapRolesBadge, CopyableId, OccupationSelect } from '@/components/tap'
import { TimeSeriesChart } from '@/components/common'
import { ConfirmDialog } from '@/components/common'
import { formatRelativeTime } from '@/lib/date'
import { ROUTES } from '@/lib/constants'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'
import { UserBadge } from '@/components/tap/user-badge'
import { StatsCard } from '@/components/dashboard/stats-card'

export const AdminTapDetailPage = () => {
    const { tapId } = useParams<{ tapId: string }>()
    const navigate = useNavigate()
    const { t, i18n } = useTranslation()

    const [deleteDialogOpen, setDeleteDialogOpen] = useState(false)

    useStatsSSE(tapId ? [tapKeys.stats(tapId)] : [])
    const { data: tap, isLoading: isLoadingTap } = useTap(tapId)
    const { data: stats, isLoading: isLoadingStats, isFetching: isStatsFetching, refetch: refetchStats } = useTapStats(tapId)

    const owner = tap?.owner
    const { mutateAsync: deleteTap, isPending: isDeleting } = useDeleteTap()
    const { mutate: updateOccupation, isPending: isUpdating } = useUpdateTapOccupation(tapId!)

    const handleOccupationChange = (occupation: any) => {
        updateOccupation(occupation, {
            onSuccess: () => {
                toast.success(t('admin.taps.updateSuccess'))
            },
            onError: () => {
                toast.error(t('errors.updateFailed'))
            },
        })
    }

    const handleDelete = async () => {

        if (!tapId) return
        await deleteTap(tapId)
        toast.success(t('admin.taps.deleteSuccess'))
        navigate(ROUTES.ADMIN_TAPS)
    }

    if (isLoadingTap) {
        return (
            <div className="space-y-6">
                <Skeleton className="h-10 w-48" />
                <Card>
                    <CardHeader>
                        <Skeleton className="h-6 w-32" />
                    </CardHeader>
                    <CardContent className="space-y-4">
                        <Skeleton className="h-20 w-full" />
                        <Skeleton className="h-20 w-full" />
                    </CardContent>
                </Card>
            </div>
        )
    }

    if (!tap) {
        return (
            <div className="flex flex-col items-center justify-center py-12">
                <h2 className="text-2xl font-semibold">{t('errors.tapNotFound')}</h2>
                <Button asChild className="mt-4" variant="outline">
                    <Link to={ROUTES.ADMIN_TAPS}>
                        <ArrowLeft className="mr-2 h-4 w-4" />
                        {t('common.back')}
                    </Link>
                </Button>
            </div>
        )
    }

    return (
        <div className="space-y-6">
            {/* Header */}
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-4">
                    <Button asChild variant="ghost" size="icon">
                        <Link to={ROUTES.ADMIN_TAPS}>
                            <ArrowLeft className="h-5 w-5" />
                        </Link>
                    </Button>
                    <div>
                        <h1 className="text-2xl font-semibold">
                            {t('admin.taps.tapDetails')}
                        </h1>
                        <p className="text-muted-foreground text-sm">
                            {t('admin.taps.manageTap')}
                        </p>
                    </div>
                </div>
                <Button variant="destructive" onClick={() => setDeleteDialogOpen(true)}>
                    <Trash2 className="mr-2 h-4 w-4" />
                    {t('admin.taps.actions.delete')}
                </Button>
            </div>

            {/* Tap Information Card */}
            <Card>
                <CardHeader>
                    <CardTitle>{t('admin.taps.information')}</CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                    <div>
                        <div className="mb-2 flex items-center gap-2">
                            <h2 className="text-xl font-semibold">{tap.name}</h2>
                            <OccupationSelect
                                value={tap.occupation}
                                onChange={handleOccupationChange}
                                disabled={isUpdating}
                            />
                        </div>
                        <p className="text-muted-foreground">{tap.description}</p>
                    </div>

                    <div className="grid gap-4 md:grid-cols-2">
                        <div>
                            <span className="text-muted-foreground text-sm">
                                {t('taps.tapId')}:
                            </span>{' '}
                            <CopyableId id={tap.id} className="mt-1" />
                        </div>
                        <div>
                            <span className="text-muted-foreground text-sm">
                                {t('taps.permission')}:
                            </span>{' '}
                            <div className="mt-1">
                                <PermissionBadge
                                    permission={tap.permission}
                                    hasAccess={tap.hasAccess}
                                />
                            </div>
                        </div>
                        <div>
                            <span className="text-muted-foreground text-sm">
                                {t('taps.roles.label')}:
                            </span>{' '}
                            <TapRolesBadge roles={tap.roles} className="mt-1" />
                        </div>
                        <div>
                            <span className="text-muted-foreground text-sm">
                                {t('taps.totalUses')}:
                            </span>{' '}
                            <div className="text-sm font-medium">
                                {tap.totalUses}
                            </div>
                        </div>
                        <div>
                            <span className="text-muted-foreground text-sm">
                                {t('taps.createdAt')}:
                            </span>{' '}
                            <div className="text-sm">
                                {formatRelativeTime(tap.createdAt, i18n.language)}
                            </div>
                        </div>
                        <div>
                            <span className="text-muted-foreground text-sm">
                                {t('taps.updatedAt')}:
                            </span>{' '}
                            <div className="text-sm">
                                {formatRelativeTime(tap.updatedAt, i18n.language)}
                            </div>
                        </div>
                    </div>

                    {/* Owner Information */}
                    <div className="border-t pt-4">
                        <h3 className="mb-3 text-sm font-medium">
                            {t('admin.taps.owner')}
                        </h3>
                        {isLoadingTap ? (
                            <Skeleton className="h-12 w-full" />
                        ) : owner ? (
                            <UserBadge user={owner} />
                        ) : (
                            <p className="text-muted-foreground text-sm">
                                {t('errors.ownerNotFound')}
                            </p>
                        )}
                    </div>
                </CardContent>
            </Card>

            {/* Statistics Overview */}
            {stats && (
                <>
                    <div className="grid gap-4 md:grid-cols-4">
                        <StatsCard
                            title={t('taps.stats.activeNow')}
                            value={stats.currentlyActive}
                            icon={<Activity className="h-4 w-4" />}
                            onRefresh={refetchStats}
                            isRefreshing={isStatsFetching}
                        />
                        <StatsCard
                            title={t('dashboard.stats.totalUses')}
                            value={stats.totalUses.toLocaleString()}
                            icon={<MousePointer2 className="h-4 w-4" />}
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
                        <StatsCard
                            title={t('taps.stats.cacheHits')}
                            value={stats.cacheHits.toLocaleString()}
                            icon={<Database className="h-4 w-4" />}
                            onRefresh={refetchStats}
                            isRefreshing={isStatsFetching}
                        />
                    </div>

                    {/* Charts */}
                    <div className="grid gap-6 lg:grid-cols-2">
                        {isLoadingStats ? (
                            <>
                                <Skeleton className="h-96 w-full" />
                                <Skeleton className="h-96 w-full" />
                            </>
                        ) : (
                            <>
                                <TimeSeriesChart
                                    title={t('taps.stats.useRate')}
                                    data={stats.useRateHistory}
                                />
                                <TimeSeriesChart
                                    title={t('taps.stats.cacheHitRate')}
                                    data={stats.cacheHitRateHistory}
                                />
                            </>
                        )}
                    </div>
                </>
            )}

            {/* Delete Confirmation Dialog */}
            <ConfirmDialog
                open={deleteDialogOpen}
                onOpenChange={setDeleteDialogOpen}
                title={t('admin.taps.deleteConfirm.title')}
                description={t('admin.taps.deleteConfirm.description', {
                    name: tap.name,
                })}
                onConfirm={handleDelete}
                isLoading={isDeleting}
                variant="destructive"
            />
        </div>
    )
}
