import { useState } from 'react'
import { useParams, Link, useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { ArrowLeft, Trash2 } from 'lucide-react'
import { useTap, useTapStats, useDeleteTap } from '@/features/taps'
import { useUserPublic } from '@/features/users'
import { PermissionBadge, OccupationBadge } from '@/components/tap'
import { TimeSeriesChart } from '@/components/common'
import { ConfirmDialog } from '@/components/common'
import { formatRelativeTime } from '@/lib/date'
import { ROUTES } from '@/lib/constants'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'

export const AdminTapDetailPage = () => {
  const { tapId } = useParams<{ tapId: string }>()
  const navigate = useNavigate()
  const { t, i18n } = useTranslation()

  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false)

  const { data: tap, isLoading: isLoadingTap } = useTap(tapId)
  const { data: stats, isLoading: isLoadingStats } = useTapStats(tapId)
  const { data: owner, isLoading: isLoadingOwner } = useUserPublic(tap?.ownerId)

  const { mutateAsync: deleteTap, isPending: isDeleting } = useDeleteTap()

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
              <OccupationBadge occupation={tap.occupation} />
            </div>
            <p className="text-muted-foreground">{tap.description}</p>
          </div>

          <div className="grid gap-4 md:grid-cols-2">
            <div>
              <span className="text-muted-foreground text-sm">
                {t('taps.tapId')}:
              </span>{' '}
              <code className="bg-muted rounded px-1 text-sm">{tap.id}</code>
            </div>
            <div>
              <span className="text-muted-foreground text-sm">
                {t('taps.permission')}:
              </span>{' '}
              <PermissionBadge
                permission={tap.permission}
                hasAccess={tap.hasAccess}
              />
            </div>
            <div>
              <span className="text-muted-foreground text-sm">
                {t('taps.roles.label')}:
              </span>{' '}
              <div className="mt-1 flex gap-2">
                {tap.roles.map((role) => (
                  <Badge key={role} variant="outline">
                    {t(`taps.roles.${role}`)}
                  </Badge>
                ))}
              </div>
            </div>
            <div>
              <span className="text-muted-foreground text-sm">
                {t('taps.totalUses')}:
              </span>{' '}
              {tap.totalUses.toLocaleString()}
            </div>
            <div>
              <span className="text-muted-foreground text-sm">
                {t('taps.createdAt')}:
              </span>{' '}
              {formatRelativeTime(tap.createdAt, i18n.language)}
            </div>
            <div>
              <span className="text-muted-foreground text-sm">
                {t('taps.updatedAt')}:
              </span>{' '}
              {formatRelativeTime(tap.updatedAt, i18n.language)}
            </div>
          </div>

          {/* Owner Information */}
          <div className="border-t pt-4">
            <h3 className="mb-3 text-sm font-medium">
              {t('admin.taps.owner')}
            </h3>
            {isLoadingOwner ? (
              <Skeleton className="h-12 w-full" />
            ) : owner ? (
              <Link
                to={ROUTES.ADMIN_USER(owner.id)}
                className="hover:bg-accent flex items-center gap-3 rounded-lg border p-3 transition-colors"
              >
                <Avatar className="h-10 w-10">
                  <AvatarImage src={owner.avatar} alt={owner.username} />
                  <AvatarFallback>
                    {owner.username[0].toUpperCase()}
                  </AvatarFallback>
                </Avatar>
                <div>
                  <p className="font-medium">{owner.username}</p>
                  <p className="text-muted-foreground text-sm">{owner.id}</p>
                </div>
              </Link>
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
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-muted-foreground text-sm font-medium">
                  {t('taps.stats.currentlyActive')}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {stats.currentlyActive}
                </div>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-muted-foreground text-sm font-medium">
                  {t('taps.stats.totalUses')}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {stats.totalUses.toLocaleString()}
                </div>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-muted-foreground text-sm font-medium">
                  {t('taps.stats.uniqueUsers')}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {stats.uniqueUsers.toLocaleString()}
                </div>
              </CardContent>
            </Card>
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-muted-foreground text-sm font-medium">
                  {t('taps.stats.cacheHits')}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {stats.cacheHits.toLocaleString()}
                </div>
              </CardContent>
            </Card>
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
          tapName: tap.name,
        })}
        onConfirm={handleDelete}
        isLoading={isDeleting}
        variant="destructive"
      />
    </div>
  )
}
