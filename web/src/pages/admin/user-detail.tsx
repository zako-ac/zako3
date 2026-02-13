import { useState } from 'react'
import { useParams, Link, useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { ArrowLeft, Ban, UserCheck, Shield, ShieldOff } from 'lucide-react'
import {
  useUser,
  useBanUser,
  useUnbanUser,
  useUpdateUserRole,
} from '@/features/users'
import { useTaps } from '@/features/taps'
import { TapCard } from '@/components/tap'
import { ConfirmDialog } from '@/components/common'
import { formatRelativeTime } from '@/lib/date'
import { ROUTES } from '@/lib/constants'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'

export const AdminUserDetailPage = () => {
  const { userId } = useParams<{ userId: string }>()
  const navigate = useNavigate()
  const { t, i18n } = useTranslation()

  const [banDialogOpen, setBanDialogOpen] = useState(false)
  const [unbanDialogOpen, setUnbanDialogOpen] = useState(false)
  const [adminDialogOpen, setAdminDialogOpen] = useState(false)

  const { data: user, isLoading: isLoadingUser } = useUser(userId)
  const { data: tapsData, isLoading: isLoadingTaps } = useTaps({
    ownerId: userId,
  })

  const { mutateAsync: banUser, isPending: isBanning } = useBanUser()
  const { mutateAsync: unbanUser, isPending: isUnbanning } = useUnbanUser()
  const { mutateAsync: updateRole, isPending: isUpdatingRole } =
    useUpdateUserRole()

  const taps = tapsData?.data ?? []

  const handleBan = async () => {
    if (!userId) return
    await banUser({ userId, reason: 'Banned by admin' })
    toast.success(t('admin.users.banSuccess'))
    setBanDialogOpen(false)
  }

  const handleUnban = async () => {
    if (!userId) return
    await unbanUser(userId)
    toast.success(t('admin.users.unbanSuccess'))
    setUnbanDialogOpen(false)
  }

  const handleToggleAdmin = async () => {
    if (!userId || !user) return
    const newIsAdmin = !user.isAdmin
    await updateRole({ userId, isAdmin: newIsAdmin })
    toast.success(
      user.isAdmin
        ? t('admin.users.removedAdminRole')
        : t('admin.users.grantedAdminRole')
    )
    setAdminDialogOpen(false)
  }

  if (isLoadingUser) {
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

  if (!user) {
    return (
      <div className="flex flex-col items-center justify-center py-12">
        <h2 className="text-2xl font-semibold">{t('errors.userNotFound')}</h2>
        <Button asChild className="mt-4" variant="outline">
          <Link to={ROUTES.ADMIN_USERS}>
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
            <Link to={ROUTES.ADMIN_USERS}>
              <ArrowLeft className="h-5 w-5" />
            </Link>
          </Button>
          <div>
            <h1 className="text-2xl font-semibold">
              {t('admin.users.userDetails')}
            </h1>
            <p className="text-muted-foreground text-sm">
              {t('admin.users.manageUser')}
            </p>
          </div>
        </div>
      </div>

      {/* User Profile Card */}
      <Card>
        <CardHeader>
          <CardTitle>{t('admin.users.profile')}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-6">
          <div className="flex items-start gap-6">
            <Avatar className="h-20 w-20">
              <AvatarImage src={user.avatar} alt={user.username} />
              <AvatarFallback>{user.username[0].toUpperCase()}</AvatarFallback>
            </Avatar>
            <div className="flex-1 space-y-3">
              <div>
                <div className="flex items-center gap-2">
                  <h2 className="text-xl font-semibold">{user.username}</h2>
                  {user.isAdmin && (
                    <Badge variant="default">
                      <Shield className="mr-1 h-3 w-3" />
                      {t('admin.users.roles.admin')}
                    </Badge>
                  )}
                  {user.isBanned && (
                    <Badge variant="destructive">
                      <Ban className="mr-1 h-3 w-3" />
                      {t('admin.users.status.banned')}
                    </Badge>
                  )}
                </div>
                <p className="text-muted-foreground text-sm">{user.email}</p>
              </div>
              <div className="grid grid-cols-2 gap-4 text-sm">
                <div>
                  <span className="text-muted-foreground">
                    {t('admin.users.userId')}:
                  </span>{' '}
                  <code className="bg-muted rounded px-1">{user.id}</code>
                </div>
                <div>
                  <span className="text-muted-foreground">
                    {t('admin.users.createdAt')}:
                  </span>{' '}
                  {formatRelativeTime(user.createdAt, i18n.language)}
                </div>
                {user.lastActiveAt && (
                  <div>
                    <span className="text-muted-foreground">
                      {t('admin.users.lastActive')}:
                    </span>{' '}
                    {formatRelativeTime(user.lastActiveAt, i18n.language)}
                  </div>
                )}
              </div>
            </div>
          </div>

          {/* Action Buttons */}
          <div className="flex gap-3 border-t pt-4">
            {user.isBanned ? (
              <Button
                variant="outline"
                onClick={() => setUnbanDialogOpen(true)}
                disabled={isUnbanning}
              >
                <UserCheck className="mr-2 h-4 w-4" />
                {t('admin.users.actions.unban')}
              </Button>
            ) : (
              <Button
                variant="destructive"
                onClick={() => setBanDialogOpen(true)}
                disabled={isBanning}
              >
                <Ban className="mr-2 h-4 w-4" />
                {t('admin.users.actions.ban')}
              </Button>
            )}
            <Button
              variant="outline"
              onClick={() => setAdminDialogOpen(true)}
              disabled={isUpdatingRole}
            >
              {user.isAdmin ? (
                <>
                  <ShieldOff className="mr-2 h-4 w-4" />
                  {t('admin.users.actions.removeAdmin')}
                </>
              ) : (
                <>
                  <Shield className="mr-2 h-4 w-4" />
                  {t('admin.users.actions.makeAdmin')}
                </>
              )}
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Owned Taps */}
      <Card>
        <CardHeader>
          <CardTitle>
            {t('admin.users.ownedTaps')} ({taps.length})
          </CardTitle>
        </CardHeader>
        <CardContent>
          {isLoadingTaps ? (
            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
              {Array.from({ length: 3 }).map((_, i) => (
                <Skeleton key={i} className="h-40 w-full" />
              ))}
            </div>
          ) : taps.length === 0 ? (
            <p className="text-muted-foreground py-8 text-center">
              {t('admin.users.noTaps')}
            </p>
          ) : (
            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
              {taps.map((tap) => (
                <TapCard
                  key={tap.id}
                  tap={tap}
                  onReport={() => {}}
                  onClick={(tapId) => navigate(ROUTES.ADMIN_TAP(tapId))}
                />
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Confirm Dialogs */}
      <ConfirmDialog
        open={banDialogOpen}
        onOpenChange={setBanDialogOpen}
        title={t('admin.users.banConfirm.title')}
        description={t('admin.users.banConfirm.description', {
          username: user.username,
        })}
        onConfirm={handleBan}
        isLoading={isBanning}
        variant="destructive"
      />

      <ConfirmDialog
        open={unbanDialogOpen}
        onOpenChange={setUnbanDialogOpen}
        title={t('admin.users.unbanConfirm.title')}
        description={t('admin.users.unbanConfirm.description', {
          username: user.username,
        })}
        onConfirm={handleUnban}
        isLoading={isUnbanning}
      />

      <ConfirmDialog
        open={adminDialogOpen}
        onOpenChange={setAdminDialogOpen}
        title={
          user.isAdmin
            ? t('admin.users.removeAdminConfirm.title')
            : t('admin.users.makeAdminConfirm.title')
        }
        description={
          user.isAdmin
            ? t('admin.users.removeAdminConfirm.description', {
                username: user.username,
              })
            : t('admin.users.makeAdminConfirm.description', {
                username: user.username,
              })
        }
        onConfirm={handleToggleAdmin}
        isLoading={isUpdatingRole}
      />
    </div>
  )
}
