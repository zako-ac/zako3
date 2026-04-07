import { useTranslation } from 'react-i18next'
import { ArrowDown, CheckCircle, XCircle } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import type { MapperStepResultDto } from '../api'

const changed = (a: string, b: string) => a !== b

interface EvaluateStepCardProps {
    step: MapperStepResultDto
    index: number
}

export const EvaluateStepCard = ({ step, index }: EvaluateStepCardProps) => {
    const { t } = useTranslation()
    const textChanged = changed(step.input_text, step.output_text)

    return (
        <div className="rounded-lg border p-3 space-y-2">
            {/* Header */}
            <div className="flex items-center gap-2">
                <span className="text-muted-foreground w-5 text-center font-mono text-xs">
                    {index + 1}
                </span>
                <span className="text-sm font-medium">{step.mapper_name}</span>
                <span className="text-muted-foreground font-mono text-xs">({step.mapper_id})</span>
                <div className="ml-auto flex items-center gap-1">
                    {textChanged && (
                        <Badge variant="outline" className="text-xs px-1.5 py-0">
                            {t('admin.mappers.pipeline.evaluate.stepChanged')}
                        </Badge>
                    )}
                    {step.success ? (
                        <CheckCircle className="h-4 w-4 text-green-500" />
                    ) : (
                        <XCircle className="h-4 w-4 text-red-500" />
                    )}
                </div>
            </div>

            {/* Input */}
            <div className="space-y-0.5">
                <div className="text-muted-foreground text-xs font-medium uppercase tracking-wide">{t('admin.mappers.pipeline.evaluate.stepInput')}</div>
                <pre className="bg-muted rounded px-2 py-1 font-mono text-xs whitespace-pre-wrap break-all">
                    {step.input_text || <span className="italic opacity-50">{t('admin.mappers.pipeline.evaluate.stepEmpty')}</span>}
                </pre>
            </div>

            {/* Arrow */}
            <div className="flex justify-center">
                <ArrowDown className={`h-3.5 w-3.5 ${textChanged ? 'text-primary' : 'text-muted-foreground'}`} />
            </div>

            {/* Output */}
            <div className="space-y-0.5">
                <div className="text-muted-foreground text-xs font-medium uppercase tracking-wide">{t('admin.mappers.pipeline.evaluate.stepOutput')}</div>
                <pre className={`rounded px-2 py-1 font-mono text-xs whitespace-pre-wrap break-all ${textChanged ? 'bg-primary/10 ring-1 ring-primary/20' : 'bg-muted'}`}>
                    {step.output_text || <span className="italic opacity-50">{t('admin.mappers.pipeline.evaluate.stepEmpty')}</span>}
                </pre>
            </div>

            {/* Error */}
            {step.error && (
                <div className="rounded bg-red-50 px-2 py-1 text-xs text-red-700 dark:bg-red-950 dark:text-red-300">
                    {step.error}
                </div>
            )}
        </div>
    )
}
