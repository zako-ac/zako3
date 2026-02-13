import { useTranslation } from 'react-i18next'
import { Link } from 'react-router-dom'
import {
  Users,
  Compass,
  AlertTriangle,
  Activity,
  Bell,
  ExternalLink,
} from 'lucide-react'
import { useUsers } from '@/features/users'
import { useTaps } from '@/features/taps'
import { useAdminActivity, usePendingVerifications } from '@/features/admin'
import { StatsCard } from '@/components/dashboard/stats-card'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { formatRelativeTime } from '@/lib/date'
import { ROUTES } from '@/lib/constants'

export const AdminDashboardPage = () => {
  const { t, i18n } = useTranslation()
  const { data: usersData, isLoading: isUsersLoading } = useUsers({
    perPage: 1,
  })
  const { data: tapsData, isLoading: isTapsLoading } = useTaps({ perPage: 1 })
  const { data: activityData, isLoading: isActivityLoading } = useAdminActivity(
    { perPage: 5 }
  )
  const { data: pendingTaps, isLoading: isPendingLoading } =
    usePendingVerifications()

  const totalUsers = usersData?.meta.total ?? 0
  const totalTaps = tapsData?.meta.total ?? 0
  const recentActivity = activityData?.data ?? []
  const pendingVerifications = pendingTaps ?? []
  const grafanaUrl = import.meta.env.VITE_GRAFANA_URL

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold">{t('admin.dashboard')}</h1>
        <p className="text-muted-foreground">{t('admin.dashboardSubtitle')}</p>
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <StatsCard
          title={t('admin.stats.totalUsers')}
          value={totalUsers.toLocaleString()}
          icon={<Users className="h-4 w-4" />}
          isLoading={isUsersLoading}
        />
        <StatsCard
          title={t('admin.stats.totalTaps')}
          value={totalTaps.toLocaleString()}
          icon={<Compass className="h-4 w-4" />}
          isLoading={isTapsLoading}
        />
        <StatsCard
          title={t('admin.stats.pendingVerifications')}
          value={
            isPendingLoading ? '--' : pendingVerifications.length.toString()
          }
          icon={<AlertTriangle className="h-4 w-4" />}
          description={t('admin.stats.requiresReview')}
          isLoading={isPendingLoading}
        />
        <StatsCard
          title={t('admin.stats.systemHealth')}
          value="Healthy"
          icon={<Activity className="h-4 w-4" />}
          description={t('admin.stats.allSystemsOperational')}
        />
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        {/* Quick Actions */}
        <Card>
          <CardHeader>
            <CardTitle>{t('admin.quickActions')}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            <Button asChild variant="outline" className="w-full justify-start">
              <Link to={ROUTES.ADMIN_NOTIFICATIONS}>
                <Bell className="mr-2 h-4 w-4" />
                {t('admin.actions.viewNotifications')}
              </Link>
            </Button>
            <Button asChild variant="outline" className="w-full justify-start">
              <Link to={ROUTES.ADMIN_TAPS}>
                <AlertTriangle className="mr-2 h-4 w-4" />
                {t('admin.actions.manageTaps')}
              </Link>
            </Button>
            {grafanaUrl && (
              <Button
                asChild
                variant="outline"
                className="w-full justify-start"
              >
                <a href={grafanaUrl} target="_blank" rel="noopener noreferrer">
                  <ExternalLink className="mr-2 h-4 w-4" />
                  {t('admin.actions.openGrafana')}
                </a>
              </Button>
            )}
          </CardContent>
        </Card>

        {/* Recent Activity */}
        <Card>
          <CardHeader>
            <CardTitle>{t('admin.recentActivity')}</CardTitle>
          </CardHeader>
          <CardContent>
            {isActivityLoading ? (
              <div className="space-y-3">
                {Array.from({ length: 5 }).map((_, i) => (
                  <Skeleton key={i} className="h-12 w-full" />
                ))}
              </div>
            ) : recentActivity.length === 0 ? (
              <p className="text-muted-foreground py-8 text-center text-sm">
                {t('admin.noRecentActivity')}
              </p>
            ) : (
              <div className="space-y-3">
                {recentActivity.map((activity) => (
                  <div
                    key={activity.id}
                    className="flex items-center justify-between text-sm"
                  >
                    <div className="flex-1">
                      <span className="font-medium">{activity.action}</span>
                      <span className="text-muted-foreground">
                        {' '}
                        on {activity.targetName}
                      </span>
                    </div>
                    <span className="text-muted-foreground text-xs">
                      {formatRelativeTime(activity.timestamp, i18n.language)}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Pending Verifications */}
      {!isPendingLoading && pendingVerifications.length > 0 && (
        <Card>
          <CardHeader>
            <div className="flex items-center justify-between">
              <CardTitle>{t('admin.pendingVerifications')}</CardTitle>
              <Button asChild variant="ghost" size="sm">
                <Link to={ROUTES.ADMIN_TAPS}>{t('common.viewAll')}</Link>
              </Button>
            </div>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              {pendingVerifications.slice(0, 5).map((tap) => (
                <div key={tap.id} className="flex items-center justify-between">
                  <Link
                    to={ROUTES.ADMIN_TAP(tap.id)}
                    className="flex-1 hover:underline"
                  >
                    {tap.name}
                  </Link>
                  <Button asChild size="sm" variant="outline">
                    <Link to={ROUTES.ADMIN_TAP(tap.id)}>
                      {t('admin.actions.review')}
                    </Link>
                  </Button>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
