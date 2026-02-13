import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { MoreHorizontal, Ban, UserCheck, Shield, ShieldOff } from 'lucide-react'
import { useUsers, useBanUser, useUnbanUser, useUpdateUserRole } from '@/features/users'
import { SearchInput, DataPagination, ConfirmDialog } from '@/components/common'
import { usePagination, useDebounce } from '@/hooks'
import { formatRelativeTime } from '@/lib/date'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
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
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Skeleton } from '@/components/ui/skeleton'
import type { User } from '@zako-ac/zako3-data'

export const AdminUsersPage = () => {
  const { t, i18n } = useTranslation()
  const { pagination, setPage, setPerPage, getPaginationInfo } = usePagination()
  const [search, setSearch] = useState('')
  const debouncedSearch = useDebounce(search, 300)

  // Dialog state
  const [banDialogOpen, setBanDialogOpen] = useState(false)
  const [unbanDialogOpen, setUnbanDialogOpen] = useState(false)
  const [selectedUser, setSelectedUser] = useState<User | null>(null)

  const { data, isLoading } = useUsers({
    page: pagination.page,
    perPage: pagination.perPage,
    search: debouncedSearch || undefined,
  })

  const { mutateAsync: banUser, isPending: isBanning } = useBanUser()
  const { mutateAsync: unbanUser, isPending: isUnbanning } = useUnbanUser()
  const { mutateAsync: updateRole, isPending: isUpdatingRole } = useUpdateUserRole()

  const users = data?.data ?? []
  const paginationInfo = getPaginationInfo(data?.meta)

  const handleBan = async () => {
    if (!selectedUser) return
    await banUser({ userId: selectedUser.id, reason: 'Banned by admin' })
    toast.success(t('admin.users.banSuccess'))
    setBanDialogOpen(false)
    setSelectedUser(null)
  }

  const handleUnban = async () => {
    if (!selectedUser) return
    await unbanUser(selectedUser.id)
    toast.success(t('admin.users.unbanSuccess'))
    setUnbanDialogOpen(false)
    setSelectedUser(null)
  }

  const handleToggleAdmin = async (user: User) => {
    const newIsAdmin = !user.isAdmin
    await updateRole({ userId: user.id, isAdmin: newIsAdmin })
    toast.success(
      user.isAdmin
        ? t('admin.users.removedAdminRole')
        : t('admin.users.grantedAdminRole')
    )
  }

  const openBanDialog = (user: User) => {
    setSelectedUser(user)
    setBanDialogOpen(true)
  }

  const openUnbanDialog = (user: User) => {
    setSelectedUser(user)
    setUnbanDialogOpen(true)
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold">{t('admin.users.title')}</h1>
        <p className="text-muted-foreground">{t('admin.users.subtitle')}</p>
      </div>

      <div className="flex items-center gap-4">
        <SearchInput
          value={search}
          onChange={setSearch}
          placeholder={t('admin.users.searchPlaceholder')}
          className="max-w-sm"
        />
      </div>

      <div className="rounded-md border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t('admin.users.user')}</TableHead>
              <TableHead>{t('admin.users.email')}</TableHead>
              <TableHead>{t('admin.users.role')}</TableHead>
              <TableHead>{t('admin.users.status')}</TableHead>
              <TableHead>{t('admin.users.lastActive')}</TableHead>
              <TableHead className="w-[50px]"></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading ? (
              Array.from({ length: 5 }).map((_, i) => (
                <TableRow key={i}>
                  <TableCell>
                    <div className="flex items-center gap-3">
                      <Skeleton className="h-8 w-8 rounded-full" />
                      <Skeleton className="h-4 w-24" />
                    </div>
                  </TableCell>
                  <TableCell><Skeleton className="h-4 w-32" /></TableCell>
                  <TableCell><Skeleton className="h-5 w-16" /></TableCell>
                  <TableCell><Skeleton className="h-5 w-16" /></TableCell>
                  <TableCell><Skeleton className="h-4 w-20" /></TableCell>
                  <TableCell><Skeleton className="h-8 w-8" /></TableCell>
                </TableRow>
              ))
            ) : users.length === 0 ? (
              <TableRow>
                <TableCell colSpan={6} className="h-24 text-center text-muted-foreground">
                  {t('admin.users.noUsers')}
                </TableCell>
              </TableRow>
            ) : (
              users.map((user) => (
                <TableRow key={user.id}>
                  <TableCell>
                    <div className="flex items-center gap-3">
                      <Avatar className="h-8 w-8">
                        <AvatarImage src={user.avatar} alt={user.username} />
                        <AvatarFallback>
                          {user.username.slice(0, 2).toUpperCase()}
                        </AvatarFallback>
                      </Avatar>
                      <div>
                        <p className="font-medium">{user.username}</p>
                        <p className="text-xs text-muted-foreground font-mono">
                          {user.id}
                        </p>
                      </div>
                    </div>
                  </TableCell>
                  <TableCell className="text-muted-foreground">
                    {user.email || '--'}
                  </TableCell>
                  <TableCell>
                    {user.isAdmin ? (
                      <Badge variant="default">Admin</Badge>
                    ) : (
                      <Badge variant="secondary">User</Badge>
                    )}
                  </TableCell>
                  <TableCell>
                    {user.isBanned ? (
                      <Badge variant="destructive">Banned</Badge>
                    ) : (
                      <Badge variant="outline" className="text-success border-success/30">
                        Active
                      </Badge>
                    )}
                  </TableCell>
                  <TableCell className="text-muted-foreground">
                    {user.lastActiveAt
                      ? formatRelativeTime(user.lastActiveAt, i18n.language)
                      : '--'}
                  </TableCell>
                  <TableCell>
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button variant="ghost" size="icon-sm">
                          <MoreHorizontal className="h-4 w-4" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem
                          onClick={() => handleToggleAdmin(user)}
                          disabled={isUpdatingRole}
                        >
                          {user.isAdmin ? (
                            <>
                              <ShieldOff className="mr-2 h-4 w-4" />
                              {t('admin.users.removeAdmin')}
                            </>
                          ) : (
                            <>
                              <Shield className="mr-2 h-4 w-4" />
                              {t('admin.users.makeAdmin')}
                            </>
                          )}
                        </DropdownMenuItem>
                        <DropdownMenuSeparator />
                        {user.isBanned ? (
                          <DropdownMenuItem
                            onClick={() => openUnbanDialog(user)}
                            className="text-success focus:text-success"
                          >
                            <UserCheck className="mr-2 h-4 w-4" />
                            {t('admin.users.unban')}
                          </DropdownMenuItem>
                        ) : (
                          <DropdownMenuItem
                            onClick={() => openBanDialog(user)}
                            className="text-destructive focus:text-destructive"
                          >
                            <Ban className="mr-2 h-4 w-4" />
                            {t('admin.users.ban')}
                          </DropdownMenuItem>
                        )}
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>

      {data?.meta && paginationInfo.totalPages > 1 && (
        <DataPagination
          meta={data.meta}
          onPageChange={setPage}
          onPerPageChange={setPerPage}
        />
      )}

      <ConfirmDialog
        open={banDialogOpen}
        onOpenChange={setBanDialogOpen}
        title={t('admin.users.banConfirmTitle')}
        description={t('admin.users.banConfirmDescription', { username: selectedUser?.username })}
        confirmLabel={t('admin.users.ban')}
        onConfirm={handleBan}
        isLoading={isBanning}
        variant="destructive"
      />

      <ConfirmDialog
        open={unbanDialogOpen}
        onOpenChange={setUnbanDialogOpen}
        title={t('admin.users.unbanConfirmTitle')}
        description={t('admin.users.unbanConfirmDescription', { username: selectedUser?.username })}
        confirmLabel={t('admin.users.unban')}
        onConfirm={handleUnban}
        isLoading={isUnbanning}
      />
    </div>
  )
}
