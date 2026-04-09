import { cn } from '@/lib/utils'

interface OnlineIndicatorProps {
    count: number
    className?: string
}

export const OnlineIndicator = ({ count, className }: OnlineIndicatorProps) => {
    const online = count > 0

    return (
        <div
            className={cn(
                'flex items-center gap-2 rounded-full px-1.5 py-0.5 text-md font-medium',
                online
                    ? 'bg-green-500/15 text-green-500'
                    : 'bg-muted text-muted-foreground',
                className
            )}
        >
            <span
                className={cn(
                    'h-4 w-4 rounded-full',
                    online ? 'bg-green-500' : 'bg-muted-foreground/50'
                )}
            />
            {count}
        </div>
    )
}
