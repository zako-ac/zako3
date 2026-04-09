import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Trash2, Check } from 'lucide-react'
import {
  useAdminNotifications,
  useMarkAsRead,
  useMarkAllAsRead,
  useDeleteNotification,
} from '@/features/notifications'
import {
  SearchInput,
  DataPagination,
  FilterDropdown,
  ConfirmDialog,
} from '@/components/common'
import { usePagination, useDebounce } from '@/hooks'
import { formatRelativeTime } from '@/lib/date'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/utils'
import type { NotificationLevel } from '@zako-ac/zako3-data'

const levelColors: Record<
  NotificationLevel,
  'default' | 'secondary' | 'outline' | 'destructive'
> = {
  info: 'default',
  success: 'secondary',
  warning: 'outline',
  error: 'destructive',
}

export const AdminNotificationsPage = () => {
  const { t, i18n } = useTranslation()
  const { pagination, setPage, setPerPage } = usePagination()
  const [search, setSearch] = useState('')
  const debouncedSearch = useDebounce(search, 300)
  const [levelFilter, setLevelFilter] = useState<NotificationLevel[]>([])
  const [readFilter, setReadFilter] = useState<boolean | undefined>()

  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false)
  const [selectedNotificationId, setSelectedNotificationId] = useState<
    string | null
  >(null)

  const { data, isLoading } = useAdminNotifications({
    page: pagination.page,
    perPage: pagination.perPage,
    search: debouncedSearch || undefined,
    level: levelFilter[0],
    isRead: readFilter,
  })

  const { mutateAsync: markAsRead } = useMarkAsRead()
  const { mutateAsync: markAllAsRead, isPending: isMarkingAll } =
    useMarkAllAsRead()
  const { mutateAsync: deleteNotification, isPending: isDeleting } =
    useDeleteNotification()

  const notifications = data?.data ?? []

  const handleMarkAsRead = async (notificationId: string) => {
    await markAsRead(notificationId)
    toast.success(t('notifications.markedAsRead'))
  }

  const handleMarkAllAsRead = async () => {
    await markAllAsRead()
    toast.success(t('notifications.markedAllAsRead'))
  }

  const handleDelete = async () => {
    if (!selectedNotificationId) return
    await deleteNotification(selectedNotificationId)
    toast.success(t('notifications.deleted'))
    setDeleteDialogOpen(false)
    setSelectedNotificationId(null)
  }

  const openDeleteDialog = (notificationId: string) => {
    setSelectedNotificationId(notificationId)
    setDeleteDialogOpen(true)
  }

  const levelOptions = [
    { label: t('notifications.levels.info'), value: 'info' },
    { label: t('notifications.levels.success'), value: 'success' },
    { label: t('notifications.levels.warning'), value: 'warning' },
    { label: t('notifications.levels.error'), value: 'error' },
  ]

  const readOptions = [
    { label: t('notifications.all'), value: 'all' },
    { label: t('notifications.read'), value: 'read' },
    { label: t('notifications.unread'), value: 'unread' },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold">
            {t('admin.notifications.title')}
          </h1>
          <p className="text-muted-foreground">
            {t('admin.notifications.subtitle')}
          </p>
        </div>
        <Button onClick={handleMarkAllAsRead} disabled={isMarkingAll}>
          <Check className="mr-2 h-4 w-4" />
          {t('notifications.markAllAsRead')}
        </Button>
      </div>

      <div className="flex items-center gap-4">
        <SearchInput
          value={search}
          onChange={setSearch}
          placeholder={t('common.search')}
          className="flex-1"
        />
        <FilterDropdown
          label={t('notifications.level')}
          options={levelOptions}
          selected={levelFilter}
          onChange={(value) => setLevelFilter(value as NotificationLevel[])}
        />
        <FilterDropdown
          label={t('notifications.status')}
          options={readOptions}
          selected={
            readFilter === undefined ? [] : readFilter ? ['read'] : ['unread']
          }
          onChange={(value) => {
            if (value.length === 0) {
              setReadFilter(undefined)
            } else {
              setReadFilter(value[0] === 'read')
            }
          }}
        />
      </div>

      <div className="rounded-md border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-[100px]">
                {t('notifications.level')}
              </TableHead>
              <TableHead className="w-[150px]">
                {t('notifications.category')}
              </TableHead>
              <TableHead>{t('notifications.message')}</TableHead>
              <TableHead className="w-[150px]">
                {t('common.timestamp')}
              </TableHead>
              <TableHead className="w-[100px]">
                {t('notifications.status')}
              </TableHead>
              <TableHead className="w-[100px]">{t('common.actions')}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading ? (
              Array.from({ length: 5 }).map((_, i) => (
                <TableRow key={i}>
                  <TableCell>
                    <Skeleton className="h-5 w-16" />
                  </TableCell>
                  <TableCell>
                    <Skeleton className="h-4 w-24" />
                  </TableCell>
                  <TableCell>
                    <Skeleton className="h-4 w-full" />
                  </TableCell>
                  <TableCell>
                    <Skeleton className="h-4 w-20" />
                  </TableCell>
                  <TableCell>
                    <Skeleton className="h-5 w-16" />
                  </TableCell>
                  <TableCell>
                    <Skeleton className="h-8 w-8" />
                  </TableCell>
                </TableRow>
              ))
            ) : notifications.length === 0 ? (
              <TableRow>
                <TableCell
                  colSpan={6}
                  className="text-muted-foreground h-24 text-center"
                >
                  {t('notifications.noNotifications')}
                </TableCell>
              </TableRow>
            ) : (
              notifications.map((notification) => (
                <TableRow
                  key={notification.id}
                  className={cn(!notification.isRead && 'bg-accent/50')}
                >
                  <TableCell>
                    <Badge variant={levelColors[notification.level]}>
                      {t(`notifications.levels.${notification.level}`)}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-sm">
                    {t(`notifications.categories.${notification.category}`)}
                  </TableCell>
                  <TableCell>
                    <div className="space-y-1">
                      <p className="font-medium">{notification.title}</p>
                      <p className="text-muted-foreground text-sm">
                        {notification.message}
                      </p>
                    </div>
                  </TableCell>
                  <TableCell className="text-muted-foreground text-sm">
                    {formatRelativeTime(notification.createdAt, i18n.language)}
                  </TableCell>
                  <TableCell>
                    {notification.isRead ? (
                      <Badge variant="outline">{t('notifications.read')}</Badge>
                    ) : (
                      <Badge variant="default">
                        {t('notifications.unread')}
                      </Badge>
                    )}
                  </TableCell>
                  <TableCell>
                    <div className="flex items-center gap-2">
                      {!notification.isRead && (
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          onClick={() => handleMarkAsRead(notification.id)}
                        >
                          <Check className="h-4 w-4" />
                        </Button>
                      )}
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        onClick={() => openDeleteDialog(notification.id)}
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>

      {!isLoading && notifications.length > 0 && data?.meta && (
        <DataPagination
          meta={data.meta}
          onPageChange={setPage}
          onPerPageChange={setPerPage}
        />
      )}

      <ConfirmDialog
        open={deleteDialogOpen}
        onOpenChange={setDeleteDialogOpen}
        title={t('notifications.deleteConfirm.title')}
        description={t('notifications.deleteConfirm.description')}
        onConfirm={handleDelete}
        isLoading={isDeleting}
      />
    </div>
  )
}
