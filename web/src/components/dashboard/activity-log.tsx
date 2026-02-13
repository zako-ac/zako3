import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { formatRelativeTime } from '@/lib/date'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Skeleton } from '@/components/ui/skeleton'
import type { NotificationLevel } from '@zako-ac/zako3-data'

export interface ActivityItem {
  id: string
  title: string
  description?: string
  level: NotificationLevel
  timestamp: string
  metadata?: Record<string, unknown>
}

const levelStyles: Record<
  NotificationLevel,
  { dot: string; border: string }
> = {
  info: { dot: 'bg-info', border: 'border-info/30' },
  success: { dot: 'bg-success', border: 'border-success/30' },
  warning: { dot: 'bg-warning', border: 'border-warning/30' },
  error: { dot: 'bg-destructive', border: 'border-destructive/30' },
}

interface ActivityLogProps {
  title?: string
  items: ActivityItem[]
  isLoading?: boolean
  maxHeight?: string
  className?: string
}

export const ActivityLog = ({
  title,
  items,
  isLoading = false,
  maxHeight = '400px',
  className,
}: ActivityLogProps) => {
  const { i18n } = useTranslation()

  if (isLoading) {
    return (
      <Card className={className}>
        {title && (
          <CardHeader>
            <CardTitle>{title}</CardTitle>
          </CardHeader>
        )}
        <CardContent>
          <div className="space-y-4">
            {Array.from({ length: 5 }).map((_, i) => (
              <div key={i} className="flex gap-3">
                <Skeleton className="h-3 w-3 rounded-full" />
                <div className="flex-1 space-y-2">
                  <Skeleton className="h-4 w-3/4" />
                  <Skeleton className="h-3 w-1/2" />
                </div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>
    )
  }

  return (
    <Card className={className}>
      {title && (
        <CardHeader>
          <CardTitle>{title}</CardTitle>
        </CardHeader>
      )}
      <CardContent>
        <ScrollArea style={{ height: maxHeight }}>
          {items.length === 0 ? (
            <div className="flex items-center justify-center py-8 text-sm text-muted-foreground">
              No activity yet
            </div>
          ) : (
            <div className="relative space-y-0">
              <div className="absolute bottom-0 left-[5px] top-0 w-px bg-border" />
              {items.map((item) => {
                const styles = levelStyles[item.level]
                return (
                  <div key={item.id} className="relative flex gap-3 pb-4 last:pb-0">
                    <div
                      className={cn(
                        'relative z-10 mt-1.5 h-3 w-3 rounded-full border-2 bg-background',
                        styles.border
                      )}
                    >
                      <div
                        className={cn('absolute inset-0.5 rounded-full', styles.dot)}
                      />
                    </div>
                    <div className="flex-1 space-y-1">
                      <div className="flex items-center justify-between gap-2">
                        <p className="text-sm font-medium leading-none">{item.title}</p>
                        <span className="shrink-0 text-xs text-muted-foreground">
                          {formatRelativeTime(item.timestamp, i18n.language)}
                        </span>
                      </div>
                      {item.description && (
                        <p className="text-sm text-muted-foreground">
                          {item.description}
                        </p>
                      )}
                    </div>
                  </div>
                )
              })}
            </div>
          )}
        </ScrollArea>
      </CardContent>
    </Card>
  )
}
