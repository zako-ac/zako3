import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
} from '@/components/ui/card'
import { ChartContainer, ChartTooltip, ChartTooltipContent, type ChartConfig } from '@/components/ui/chart'
import { Line, LineChart, XAxis, CartesianGrid } from 'recharts'
import { Zap } from 'lucide-react'
import { cn } from '@/lib/utils'

type TimeRange = '10m' | '1h' | '1d' | '30d'

const TIME_RANGES: { label: string; value: TimeRange; minutes: number }[] = [
    { label: '10m', value: '10m', minutes: 10 },
    { label: '1h', value: '1h', minutes: 60 },
    { label: '1d', value: '1d', minutes: 60 * 24 },
    { label: '30d', value: '30d', minutes: 60 * 24 * 30 },
]

interface TimeRangeSelectorProps {
    value: TimeRange
    onChange: (value: TimeRange) => void
}

const TimeRangeSelector = ({ value, onChange }: TimeRangeSelectorProps) => (
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

interface TimeSeriesChartProps {
    title: string
    description?: string
    data: Array<{ timestamp: string; value: number }>
    valueFormatter?: (value: number) => string
}

const chartConfig = {
    value: {
        label: 'Value',
        color: 'var(--chart-1)',
    },
} satisfies ChartConfig

const formatTime = (timestamp: string, range: TimeRange): string => {
    const d = new Date(timestamp)
    if (range === '30d') {
        return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
    }
    return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })
}

export const TimeSeriesChart = ({
    title,
    description,
    data,
    valueFormatter = (value) => value.toString(),
}: TimeSeriesChartProps) => {
    const { t } = useTranslation()
    const [timeRange, setTimeRange] = useState<TimeRange>('1d')

    const cutoff = Date.now() - TIME_RANGES.find((r) => r.value === timeRange)!.minutes * 60 * 1000
    const formattedData = data
        .filter((p) => new Date(p.timestamp).getTime() >= cutoff)
        .map((p) => ({ ...p, time: formatTime(p.timestamp, timeRange) }))

    return (
        <Card>
            <CardHeader className="flex flex-row items-center justify-between pb-2">
                <div>
                    <CardTitle>{title}</CardTitle>
                    {description && <CardDescription>{description}</CardDescription>}
                </div>
                <TimeRangeSelector value={timeRange} onChange={setTimeRange} />
            </CardHeader>
            <CardContent>
                {formattedData.length === 0 ? (
                    <div className="flex h-[160px] flex-col items-center justify-center gap-2 text-center">
                        <Zap className="h-8 w-8 text-muted-foreground/40" />
                        <div>
                            <p className="text-sm font-medium text-muted-foreground">{t('taps.stats.noData')}</p>
                            <p className="text-xs text-muted-foreground/60">{t('taps.stats.noDataSubtext')}</p>
                        </div>
                    </div>
                ) : (
                <ChartContainer config={chartConfig}>
                    <LineChart
                        accessibilityLayer
                        data={formattedData}
                        margin={{ left: 12, right: 12 }}
                    >
                        <CartesianGrid vertical={false} />
                        <XAxis
                            dataKey="time"
                            tickLine={false}
                            axisLine={false}
                            tickMargin={8}
                        />
                        <ChartTooltip
                            cursor={false}
                            content={
                                <ChartTooltipContent
                                    formatter={(value) => [valueFormatter(value as number), '']}
                                    hideLabel
                                />
                            }
                        />
                        <Line
                            dataKey="value"
                            type="natural"
                            stroke="var(--color-value)"
                            strokeWidth={2}
                            dot={false}
                        />
                    </LineChart>
                </ChartContainer>
                )}
            </CardContent>
        </Card>
    )
}
