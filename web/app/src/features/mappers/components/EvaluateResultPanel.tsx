import { useTranslation } from 'react-i18next'
import type { EvaluateResultDto } from '../api'
import { EvaluateStepCard } from './EvaluateStepCard'

interface EvaluateResultPanelProps {
    result: EvaluateResultDto
}

export const EvaluateResultPanel = ({ result }: EvaluateResultPanelProps) => {
    const { t } = useTranslation()

    if (result.steps.length === 0) {
        return (
            <div className="text-muted-foreground rounded-lg border border-dashed py-6 text-center text-sm">
                {t('admin.mappers.pipeline.evaluate.noSteps')}
            </div>
        )
    }

    return (
        <div className="space-y-3">
            {result.steps.map((step, i) => (
                <EvaluateStepCard key={step.mapper_id + i} step={step} index={i} />
            ))}

            {/* Final output */}
            <div className="rounded-lg border-2 border-primary/30 bg-primary/5 p-3 space-y-1">
                <div className="text-xs font-semibold uppercase tracking-wide text-primary/70">
                    {t('admin.mappers.pipeline.evaluate.finalOutput')}
                </div>
                <pre className="font-mono text-sm font-medium whitespace-pre-wrap break-all">
                    {result.final_text || <span className="italic opacity-50">{t('admin.mappers.pipeline.evaluate.stepEmpty')}</span>}
                </pre>
            </div>
        </div>
    )
}
