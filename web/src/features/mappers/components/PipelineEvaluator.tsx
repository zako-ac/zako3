import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { PlayCircle, ChevronDown, ChevronUp } from 'lucide-react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import { useEvaluatePipeline } from '../hooks'
import type { EvaluateResultDto } from '../api'
import { EvaluateResultPanel } from './EvaluateResultPanel'

interface PipelineEvaluatorProps {
    /** The current (possibly unsaved) active mapper IDs in order */
    activeIds: string[]
}

export const PipelineEvaluator = ({ activeIds }: PipelineEvaluatorProps) => {
    const { t } = useTranslation()
    const [open, setOpen] = useState(false)
    const [text, setText] = useState('')
    const [result, setResult] = useState<EvaluateResultDto | null>(null)

    const { mutateAsync: evaluate, isPending } = useEvaluatePipeline()

    const handleRun = async () => {
        if (!text.trim()) return
        try {
            const res = await evaluate({ text: text.trim(), mapper_ids: activeIds })
            setResult(res)
        } catch {
            toast.error(t('admin.mappers.pipeline.evaluate.failed'))
        }
    }

    return (
        <div className="rounded-lg border">
            {/* Collapsible header */}
            <button
                type="button"
                onClick={() => setOpen((v) => !v)}
                className="flex w-full items-center justify-between px-4 py-3 text-left hover:bg-muted/50 rounded-lg"
            >
                <div className="flex items-center gap-2">
                    <PlayCircle className="h-4 w-4 text-primary" />
                    <span className="text-sm font-medium">{t('admin.mappers.pipeline.evaluate.title')}</span>
                    {activeIds.length === 0 && (
                        <span className="text-muted-foreground text-xs">{t('admin.mappers.pipeline.evaluate.noActiveMappers')}</span>
                    )}
                </div>
                {open ? <ChevronUp className="h-4 w-4" /> : <ChevronDown className="h-4 w-4" />}
            </button>

            {open && (
                <div className="border-t px-4 pb-4 pt-3 space-y-4">
                    <p className="text-muted-foreground text-xs">
                        {t('admin.mappers.pipeline.evaluate.description')}
                    </p>

                    <div className="space-y-1.5">
                        <Label htmlFor="eval-text">{t('admin.mappers.pipeline.evaluate.inputLabel')}</Label>
                        <Textarea
                            id="eval-text"
                            value={text}
                            onChange={(e) => {
                                setText(e.target.value)
                                setResult(null)
                            }}
                            placeholder={t('admin.mappers.pipeline.evaluate.inputPlaceholder')}
                            rows={3}
                            className="font-mono text-sm"
                        />
                    </div>

                    <Button
                        onClick={handleRun}
                        disabled={isPending || !text.trim() || activeIds.length === 0}
                        size="sm"
                    >
                        {isPending ? t('admin.mappers.pipeline.evaluate.running') : t('admin.mappers.pipeline.evaluate.run')}
                    </Button>

                    {result && <EvaluateResultPanel result={result} />}
                </div>
            )}
        </div>
    )
}
