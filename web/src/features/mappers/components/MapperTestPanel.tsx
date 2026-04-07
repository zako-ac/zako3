import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { Label } from '@/components/ui/label'
import type { WasmMapperDto, EvaluateResultDto } from '../api'
import { useEvaluateMapper } from '../hooks'
import { EvaluateResultPanel } from './EvaluateResultPanel'

interface MapperTestPanelProps {
    mapper: WasmMapperDto
}

export const MapperTestPanel = ({ mapper }: MapperTestPanelProps) => {
    const { t } = useTranslation()
    const [text, setText] = useState('')
    const [result, setResult] = useState<EvaluateResultDto | null>(null)

    const { mutateAsync: evalMapper, isPending } = useEvaluateMapper()

    const handleRun = async () => {
        if (!text.trim()) return
        try {
            const res = await evalMapper({ id: mapper.id, text: text.trim() })
            setResult(res)
        } catch {
            toast.error(t('admin.mappers.test.failed'))
        }
    }

    return (
        <div className="border-t px-4 pb-3 pt-3 space-y-3 bg-muted/30">
            <div className="space-y-1.5">
                <Label htmlFor={`test-${mapper.id}`} className="text-xs">
                    {t('admin.mappers.test.inputLabel')}
                </Label>
                <Textarea
                    id={`test-${mapper.id}`}
                    value={text}
                    onChange={(e) => {
                        setText(e.target.value)
                        setResult(null)
                    }}
                    placeholder={t('admin.mappers.test.inputPlaceholder')}
                    rows={2}
                    className="font-mono text-sm"
                />
            </div>
            <Button
                size="sm"
                onClick={handleRun}
                disabled={isPending || !text.trim()}
            >
                {isPending ? t('admin.mappers.test.running') : t('admin.mappers.test.run')}
            </Button>
            {result && <EvaluateResultPanel result={result} />}
        </div>
    )
}
