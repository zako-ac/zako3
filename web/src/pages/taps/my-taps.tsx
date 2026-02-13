import { useState, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate, Link } from 'react-router-dom'
import { Plus, Compass } from 'lucide-react'
import { toast } from 'sonner'
import { useMyTaps, useDeleteTap } from '@/features/taps'
import { useAuthStore } from '@/features/auth'
import { TapList } from '@/components/tap/tap-list'
import {
  ConfirmDialog,
  DataPagination,
  LoadingSkeleton,
} from '@/components/common'
import { Button } from '@/components/ui/button'
import { usePagination } from '@/hooks'
import { ROUTES } from '@/lib/constants'
import type { Tap, TapWithAccess } from '@zako-ac/zako3-data'

export const MyTapsPage = () => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { user } = useAuthStore()
  const { pagination, setPage, setPerPage, getPaginationInfo } = usePagination()

  // Delete confirmation state
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false)
  const [tapToDelete, setTapToDelete] = useState<Tap | null>(null)

  const { data, isLoading } = useMyTaps({
    page: pagination.page,
    perPage: pagination.perPage,
  })
  const { mutateAsync: deleteTap, isPending: isDeleting } = useDeleteTap()

  const taps = data?.data ?? []
  const paginationInfo = getPaginationInfo(data?.meta)

  // Transform Tap[] to TapWithAccess[] by adding owner and hasAccess
  const tapsWithAccess: TapWithAccess[] = useMemo(() => {
    if (!user) return []
    return taps.map((tap) => ({
      ...tap,
      hasAccess: true,
      owner: {
        id: user.id,
        username: user.username,
        avatar: user.avatar,
      },
    }))
  }, [taps, user])

  const handleTapClick = (tapId: string) => {
    navigate(ROUTES.TAP_STATS(tapId))
  }

  const handleSettingsClick = (tapId: string) => {
    navigate(ROUTES.TAP_SETTINGS(tapId))
  }

  const handleReport = () => {
    // No report functionality for own taps
  }

  const handleDelete = async () => {
    if (!tapToDelete) return
    await deleteTap(tapToDelete.id)
    toast.success(t('taps.deleteSuccess'))
    setDeleteDialogOpen(false)
    setTapToDelete(null)
  }

  if (isLoading) {
    return (
      <div className="space-y-6">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-semibold">{t('taps.myTaps')}</h1>
            <p className="text-muted-foreground">
              {t('taps.myTapsDescription')}
            </p>
          </div>
        </div>
        <LoadingSkeleton count={6} variant="card" />
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold">{t('taps.myTaps')}</h1>
          <p className="text-muted-foreground">{t('taps.myTapsDescription')}</p>
        </div>
        <Button asChild>
          <Link to={ROUTES.TAPS_CREATE}>
            <Plus className="mr-2 h-4 w-4" />
            {t('taps.create')}
          </Link>
        </Button>
      </div>

      {taps.length === 0 ? (
        <div className="flex flex-col items-center justify-center rounded-lg border border-dashed p-12 text-center">
          <Compass className="text-muted-foreground mb-4 h-12 w-12" />
          <h3 className="text-lg font-semibold">{t('taps.noTapsOwned')}</h3>
          <p className="text-muted-foreground mb-4">
            {t('taps.createFirstDescription')}
          </p>
          <Button asChild>
            <Link to={ROUTES.TAPS_CREATE}>
              <Plus className="mr-2 h-4 w-4" />
              {t('taps.createFirst')}
            </Link>
          </Button>
        </div>
      ) : (
        <>
          <TapList
            taps={tapsWithAccess}
            isLoading={false}
            onReport={handleReport}
            onTapClick={handleTapClick}
            onSettingsClick={handleSettingsClick}
          />

          {data?.meta && paginationInfo.totalPages > 1 && (
            <DataPagination
              meta={data.meta}
              onPageChange={setPage}
              onPerPageChange={setPerPage}
            />
          )}
        </>
      )}

      <ConfirmDialog
        open={deleteDialogOpen}
        onOpenChange={setDeleteDialogOpen}
        title={t('taps.deleteConfirmTitle')}
        description={t('taps.deleteConfirmDescription', {
          name: tapToDelete?.name,
        })}
        confirmLabel={t('common.delete')}
        onConfirm={handleDelete}
        isLoading={isDeleting}
        variant="destructive"
      />
    </div>
  )
}
