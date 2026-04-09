import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { ArrowUp, ArrowDown, Plus, Minus } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { useMappers, usePipeline, useSetPipeline } from '../hooks'
import type { WasmMapperDto } from '../api'
import { PipelineEvaluator } from './PipelineEvaluator'

export const PipelineManager = () => {
    const { t } = useTranslation()
    const { data: mappers } = useMappers()
    const { data: pipeline } = usePipeline()
    const { mutateAsync: setPipeline, isPending: isSaving } = useSetPipeline()

    // null = no local edits (show server state); string[] = user has made changes
    const [localOrder, setLocalOrder] = useState<string[] | null>(null)
    const dirty = localOrder !== null

    const serverIds = pipeline?.mapper_ids ?? []
    const activeIds = localOrder ?? serverIds

    // Helper: initialize local edit from server state if not already editing
    const edit = (updater: (prev: string[]) => string[]) => {
        setLocalOrder((prev) => updater(prev ?? serverIds))
    }

    const allMappers = mappers ?? []
    const activeMappers = activeIds
        .map((id) => allMappers.find((m) => m.id === id))
        .filter((m): m is WasmMapperDto => m !== undefined)
    const inactiveMappers = allMappers.filter((m) => !activeIds.includes(m.id))

    const enable = (id: string) => edit((prev) => [...prev, id])
    const disable = (id: string) => edit((prev) => prev.filter((v) => v !== id))

    const moveUp = (index: number) => {
        if (index === 0) return
        edit((prev) => {
            const next = [...prev]
            ;[next[index - 1], next[index]] = [next[index], next[index - 1]]
            return next
        })
    }

    const moveDown = (index: number) => {
        if (index >= activeIds.length - 1) return
        edit((prev) => {
            const next = [...prev]
            ;[next[index], next[index + 1]] = [next[index + 1], next[index]]
            return next
        })
    }

    const handleSave = async () => {
        try {
            await setPipeline({ mapper_ids: activeIds })
            toast.success(t('admin.mappers.pipeline.saveSuccess'))
            setLocalOrder(null)
        } catch {
            toast.error(t('admin.mappers.pipeline.saveError'))
        }
    }

    const handleReset = () => setLocalOrder(null)

    return (
        <div className="space-y-6">
            <div className="flex items-center justify-between">
                <div>
                    <h2 className="text-lg font-semibold">{t('admin.mappers.pipeline.pipelineOrder')}</h2>
                    <p className="text-muted-foreground text-sm">
                        {t('admin.mappers.pipeline.pipelineOrderSubtitle')}
                    </p>
                </div>
                <div className="flex gap-2">
                    {dirty && (
                        <Button variant="ghost" onClick={handleReset} disabled={isSaving}>
                            {t('admin.mappers.pipeline.reset')}
                        </Button>
                    )}
                    <Button onClick={handleSave} disabled={!dirty || isSaving}>
                        {isSaving ? t('admin.mappers.pipeline.saving') : t('admin.mappers.pipeline.savePipeline')}
                    </Button>
                </div>
            </div>

            <div className="grid gap-6 lg:grid-cols-2">
                {/* Active mappers */}
                <Card>
                    <CardHeader className="pb-3">
                        <CardTitle className="flex items-center gap-2 text-sm font-medium">
                            {t('admin.mappers.pipeline.active')}
                            <Badge variant="secondary">{activeMappers.length}</Badge>
                        </CardTitle>
                    </CardHeader>
                    <CardContent className="space-y-2">
                        {activeMappers.length === 0 && (
                            <p className="text-muted-foreground py-4 text-center text-sm">
                                {t('admin.mappers.pipeline.noActive')}
                            </p>
                        )}
                        {activeMappers.map((mapper, index) => (
                            <div
                                key={mapper.id}
                                className="flex items-center gap-2 rounded-md border p-2"
                            >
                                <span className="text-muted-foreground w-5 text-center font-mono text-xs">
                                    {index + 1}
                                </span>
                                <div className="min-w-0 flex-1">
                                    <div className="truncate text-sm font-medium">{mapper.name}</div>
                                    <div className="text-muted-foreground truncate font-mono text-xs">
                                        {mapper.id}
                                    </div>
                                </div>
                                <div className="flex gap-1">
                                    <Button
                                        variant="ghost"
                                        size="icon"
                                        className="h-7 w-7"
                                        onClick={() => moveUp(index)}
                                        disabled={index === 0}
                                    >
                                        <ArrowUp className="h-3 w-3" />
                                    </Button>
                                    <Button
                                        variant="ghost"
                                        size="icon"
                                        className="h-7 w-7"
                                        onClick={() => moveDown(index)}
                                        disabled={index === activeMappers.length - 1}
                                    >
                                        <ArrowDown className="h-3 w-3" />
                                    </Button>
                                    <Button
                                        variant="ghost"
                                        size="icon"
                                        className="text-destructive hover:text-destructive h-7 w-7"
                                        onClick={() => disable(mapper.id)}
                                        title={t('admin.mappers.pipeline.removeFromPipeline')}
                                    >
                                        <Minus className="h-3 w-3" />
                                    </Button>
                                </div>
                            </div>
                        ))}
                    </CardContent>
                </Card>

                {/* Inactive mappers */}
                <Card>
                    <CardHeader className="pb-3">
                        <CardTitle className="flex items-center gap-2 text-sm font-medium">
                            {t('admin.mappers.pipeline.inactive')}
                            <Badge variant="outline">{inactiveMappers.length}</Badge>
                        </CardTitle>
                    </CardHeader>
                    <CardContent className="space-y-2">
                        {inactiveMappers.length === 0 && (
                            <p className="text-muted-foreground py-4 text-center text-sm">
                                {t('admin.mappers.pipeline.allActive')}
                            </p>
                        )}
                        {inactiveMappers.map((mapper) => (
                            <div
                                key={mapper.id}
                                className="flex items-center gap-2 rounded-md border p-2 opacity-60"
                            >
                                <div className="min-w-0 flex-1">
                                    <div className="truncate text-sm font-medium">{mapper.name}</div>
                                    <div className="text-muted-foreground truncate font-mono text-xs">
                                        {mapper.id}
                                    </div>
                                </div>
                                <Button
                                    variant="ghost"
                                    size="icon"
                                    className="h-7 w-7"
                                    onClick={() => enable(mapper.id)}
                                    title={t('admin.mappers.pipeline.addToPipeline')}
                                >
                                    <Plus className="h-3 w-3" />
                                </Button>
                            </div>
                        ))}
                    </CardContent>
                </Card>
            </div>

            <PipelineEvaluator activeIds={activeIds} />
        </div>
    )
}
