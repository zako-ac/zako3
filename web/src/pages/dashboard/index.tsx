import { useTranslation } from 'react-i18next'
import { Link } from 'react-router-dom'
import { Compass, Plus, TrendingUp, Users, Activity } from 'lucide-react'
import { useMyTaps } from '@/features/taps'
import { useNotifications } from '@/features/notifications'
import { useAuthStore } from '@/features/auth'
import { StatsCard } from '@/components/dashboard/stats-card'
import { ActivityLog, type ActivityItem } from '@/components/dashboard/activity-log'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { ROUTES } from '@/lib/constants'

export const DashboardPage = () => {
  const { t } = useTranslation()
  const { user } = useAuthStore()
  const { data: myTapsData, isLoading: isTapsLoading } = useMyTaps({ perPage: 5 })
  const { data: notificationsData, isLoading: isNotificationsLoading } = useNotifications({ perPage: 10 })

  const myTaps = myTapsData?.data ?? []
  const totalTapUses = myTaps.reduce((sum, tap) => sum + tap.totalUses, 0)

  const activityItems: ActivityItem[] = (notificationsData?.data ?? []).map((n) => ({
    id: n.id,
    title: n.title,
    description: n.message,
    level: n.level,
    timestamp: n.createdAt,
  }))

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold">{t('dashboard.welcome', { name: user?.username ?? 'User' })}</h1>
        <p className="text-muted-foreground">{t('dashboard.subtitle')}</p>
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <StatsCard
          title={t('dashboard.stats.myTaps')}
          value={myTaps.length}
          icon={<Compass className="h-4 w-4" />}
          description={t('dashboard.stats.tapsOwned')}
          isLoading={isTapsLoading}
        />
        <StatsCard
          title={t('dashboard.stats.totalUses')}
          value={totalTapUses.toLocaleString()}
          icon={<TrendingUp className="h-4 w-4" />}
          description={t('dashboard.stats.acrossTaps')}
          isLoading={isTapsLoading}
        />
        <StatsCard
          title={t('dashboard.stats.activeUsers')}
          value="--"
          icon={<Users className="h-4 w-4" />}
          description={t('dashboard.stats.last24h')}
        />
        <StatsCard
          title={t('dashboard.stats.uptime')}
          value="99.9%"
          icon={<Activity className="h-4 w-4" />}
          trend={{ value: 0.1, isPositive: true }}
          description={t('dashboard.stats.lastMonth')}
        />
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between">
            <div>
              <CardTitle>{t('dashboard.myTaps')}</CardTitle>
              <CardDescription>{t('dashboard.recentTaps')}</CardDescription>
            </div>
            <Button asChild variant="outline" size="sm">
              <Link to={ROUTES.TAPS_CREATE}>
                <Plus className="mr-2 h-4 w-4" />
                {t('taps.create')}
              </Link>
            </Button>
          </CardHeader>
          <CardContent>
            {isTapsLoading ? (
              <div className="space-y-3">
                {Array.from({ length: 3 }).map((_, i) => (
                  <div key={i} className="h-12 rounded-md bg-muted animate-pulse" />
                ))}
              </div>
            ) : myTaps.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-8 text-center">
                <Compass className="h-12 w-12 text-muted-foreground mb-4" />
                <p className="text-sm text-muted-foreground mb-4">
                  {t('dashboard.noTapsYet')}
                </p>
                <Button asChild>
                  <Link to={ROUTES.TAPS_CREATE}>
                    <Plus className="mr-2 h-4 w-4" />
                    {t('taps.createFirst')}
                  </Link>
                </Button>
              </div>
            ) : (
              <div className="space-y-2">
                {myTaps.map((tap) => (
                  <Link
                    key={tap.id}
                    to={ROUTES.TAP_SETTINGS(tap.id)}
                    className="flex items-center justify-between rounded-lg border p-3 hover:bg-accent transition-colors"
                  >
                    <div>
                      <p className="font-medium">{tap.name}</p>
                      <p className="text-xs text-muted-foreground font-mono">{tap.id}</p>
                    </div>
                    <p className="text-sm text-muted-foreground">
                      {tap.totalUses.toLocaleString()} uses
                    </p>
                  </Link>
                ))}
                <Button asChild variant="ghost" className="w-full mt-2">
                  <Link to={ROUTES.TAPS_MINE}>{t('common.viewAll')}</Link>
                </Button>
              </div>
            )}
          </CardContent>
        </Card>

        <ActivityLog
          title={t('dashboard.recentActivity')}
          items={activityItems}
          isLoading={isNotificationsLoading}
          maxHeight="320px"
        />
      </div>
    </div>
  )
}
