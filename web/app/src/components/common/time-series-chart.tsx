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
import { Area, AreaChart, XAxis, CartesianGrid } from 'recharts'
import { Zap } from 'lucide-react'
import { TimeRangeSelector, TIME_RANGES, type TimeRange } from './time-range-selector'

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
                        <AreaChart
                            accessibilityLayer
                            data={formattedData}
                            margin={{ left: 12, right: 12 }}
                        >
                            <defs>
                                <linearGradient id="colorValue" x1="0" y1="0" x2="0" y2="1">
                                    <stop offset="5%" stopColor="#eb3489" stopOpacity={0.3} />
                                    <stop offset="95%" stopColor="#eb3489" stopOpacity={0} />
                                </linearGradient>
                            </defs>
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
                            <Area
                                dataKey="value"
                                type="natural"
                                stroke="#eb3489"
                                strokeWidth={2}
                                fill="url(#colorValue)"
                                dot={false}
                                isAnimationActive={false}
                            />
                        </AreaChart>
                    </ChartContainer>
                )}
            </CardContent>
        </Card>
    )
}
