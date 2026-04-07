import { cn } from '@/lib/utils'

export type TimeRange = '10m' | '1h' | '1d' | '30d'

export const TIME_RANGES: { label: string; value: TimeRange; minutes: number }[] = [
    { label: '10m', value: '10m', minutes: 10 },
    { label: '1h', value: '1h', minutes: 60 },
    { label: '1d', value: '1d', minutes: 60 * 24 },
    { label: '30d', value: '30d', minutes: 60 * 24 * 30 },
]

export interface TimeRangeSelectorProps {
    value: TimeRange
    onChange: (value: TimeRange) => void
}

export const TimeRangeSelector = ({ value, onChange }: TimeRangeSelectorProps) => (
    <div className="flex items-center gap-0.5 rounded-md border p-0.5">
        {TIME_RANGES.map((r) => (
            <button
                key={r.value}
                onClick={() => onChange(r.value)}
                className={cn(
                    'rounded-xs px-2 py-0.5 text-xs font-medium transition-colors',
                    value === r.value
                        ? 'bg-primary text-primary-foreground'
                        : 'text-muted-foreground hover:text-foreground'
                )}
            >
                {r.label}
            </button>
        ))}
    </div>
)
