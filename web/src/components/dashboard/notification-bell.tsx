import { useTranslation } from 'react-i18next'
import { Bell } from 'lucide-react'
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Badge } from '@/components/ui/badge'
import { useNotifications, useUnreadCount, useMarkAsRead } from '@/features/notifications'
import { formatRelativeTime } from '@/lib/date'
import { cn } from '@/lib/utils'
import type { NotificationLevel } from '@zako-ac/zako3-data'

const levelColors: Record<NotificationLevel, string> = {
  info: 'bg-info text-info-foreground',
  success: 'bg-success text-success-foreground',
  warning: 'bg-warning text-warning-foreground',
  error: 'bg-destructive text-destructive-foreground',
}

export const NotificationBell = () => {
  const { t, i18n } = useTranslation()
  const { data: unreadData } = useUnreadCount()
  const { data: notificationsData } = useNotifications({ perPage: 5 })
  const { mutate: markAsRead } = useMarkAsRead()

  const unreadCount = unreadData?.count ?? 0
  const notifications = notificationsData?.data ?? []

  const handleNotificationClick = (notificationId: string, isRead: boolean) => {
    if (!isRead) {
      markAsRead(notificationId)
    }
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" className="relative" aria-label="Notifications">
          <Bell className="h-5 w-5" />
          {unreadCount > 0 && (
            <span className="absolute -right-0.5 -top-0.5 flex h-4 w-4 items-center justify-center rounded-full bg-primary text-[10px] font-medium text-primary-foreground">
              {unreadCount > 99 ? '99+' : unreadCount}
            </span>
          )}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-80">
        <DropdownMenuLabel className="flex items-center justify-between">
          {t('notifications.title')}
          {unreadCount > 0 && (
            <Badge variant="secondary" className="text-xs">
              {unreadCount} new
            </Badge>
          )}
        </DropdownMenuLabel>
        <DropdownMenuSeparator />
        <ScrollArea className="h-[300px]">
          {notifications.length === 0 ? (
            <div className="flex items-center justify-center py-8 text-sm text-muted-foreground">
              {t('notifications.noNotifications')}
            </div>
          ) : (
            notifications.map((notification) => (
              <DropdownMenuItem
                key={notification.id}
                className={cn(
                  'flex flex-col items-start gap-1 p-3',
                  !notification.isRead && 'bg-accent/50'
                )}
                onClick={() => handleNotificationClick(notification.id, notification.isRead)}
              >
                <div className="flex w-full items-center gap-2">
                  <div
                    className={cn('h-2 w-2 rounded-full', levelColors[notification.level])}
                  />
                  <span className="flex-1 truncate font-medium">{notification.title}</span>
                  <span className="text-xs text-muted-foreground">
                    {formatRelativeTime(notification.createdAt, i18n.language)}
                  </span>
                </div>
                <p className="line-clamp-2 text-xs text-muted-foreground">
                  {notification.message}
                </p>
              </DropdownMenuItem>
            ))
          )}
        </ScrollArea>
        <DropdownMenuSeparator />
        <DropdownMenuItem asChild className="justify-center">
          <Link to="/dashboard" className="text-sm font-medium text-primary">
            View all notifications
          </Link>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
