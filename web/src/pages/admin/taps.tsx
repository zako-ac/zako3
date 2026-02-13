import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { useTaps, useReportTap } from '@/features/taps'
import type { ReportTapInput } from '@/features/taps'
import { TapList } from '@/components/tap/tap-list'
import { TapFiltersComponent as TapFilters } from '@/components/tap/tap-filters'
import { ReportModal } from '@/components/tap/report-modal'
import { DataPagination } from '@/components/common'
import { usePagination } from '@/hooks'
import { ROUTES } from '@/lib/constants'
import type { TapRole, TapSort, TapWithAccess } from '@zako-ac/zako3-data'

export const AdminTapsPage = () => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { pagination, setPage, setPerPage, getPaginationInfo } = usePagination()

  // Filter state
  const [search, setSearch] = useState('')
  const [roles, setRoles] = useState<TapRole[]>([])
  const [accessible, setAccessible] = useState<boolean | undefined>()
  const [sortField, setSortField] = useState<TapSort['field']>('recentlyCreated')
  const [sortDirection, setSortDirection] = useState<TapSort['direction']>('desc')

  // Report modal state
  const [reportModalOpen, setReportModalOpen] = useState(false)
  const [selectedTap, setSelectedTap] = useState<TapWithAccess | null>(null)

  const { data, isLoading } = useTaps({
    page: pagination.page,
    perPage: pagination.perPage,
    search: search || undefined,
    roles: roles.length > 0 ? roles : undefined,
    accessible,
    sortField,
    sortDirection,
  })

  const { mutateAsync: reportTap } = useReportTap()

  const taps = data?.data ?? []
  const paginationInfo = getPaginationInfo(data?.meta)

  const handleTapClick = (tapId: string) => {
    navigate(ROUTES.ADMIN_TAP(tapId))
  }

  const handleReport = (tapId: string) => {
    const tap = taps.find((t) => t.id === tapId)
    if (tap) {
      setSelectedTap(tap)
      setReportModalOpen(true)
    }
  }

  const handleReportSubmit = async (reportData: ReportTapInput) => {
    if (!selectedTap) return
    await reportTap({ tapId: selectedTap.id, data: reportData })
    toast.success(t('taps.report.success'))
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold">{t('admin.taps.title')}</h1>
        <p className="text-muted-foreground">{t('admin.taps.subtitle')}</p>
      </div>

      <TapFilters
        search={search}
        onSearchChange={setSearch}
        roles={roles}
        onRolesChange={setRoles}
        accessible={accessible}
        onAccessibleChange={setAccessible}
        sortField={sortField}
        sortDirection={sortDirection}
        onSortFieldChange={setSortField}
        onSortDirectionChange={setSortDirection}
      />

      <TapList
        taps={taps}
        isLoading={isLoading}
        onReport={handleReport}
        onTapClick={handleTapClick}
      />

      {data?.meta && paginationInfo.totalPages > 1 && (
        <DataPagination
          meta={data.meta}
          onPageChange={setPage}
          onPerPageChange={setPerPage}
        />
      )}

      {selectedTap && (
        <ReportModal
          open={reportModalOpen}
          onOpenChange={setReportModalOpen}
          tapId={selectedTap.id}
          tapName={selectedTap.name}
          onSubmit={handleReportSubmit}
        />
      )}
    </div>
  )
}
