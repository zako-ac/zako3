import { useTranslation } from 'react-i18next'
import { Trash2, Plus } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import { Label } from '@/components/ui/label'
import type { TextMappingRule } from './types'

interface TextMappingsFieldProps {
    value: TextMappingRule[]
    onChange: (value: TextMappingRule[]) => void
}

export function TextMappingsField({ value, onChange }: TextMappingsFieldProps) {
    const { t } = useTranslation()

    const update = (index: number, patch: Partial<TextMappingRule>) => {
        onChange(value.map((rule, i) => (i === index ? { ...rule, ...patch } : rule)))
    }

    const remove = (index: number) => {
        onChange(value.filter((_, i) => i !== index))
    }

    const add = () => {
        onChange([...value, { pattern: '', replacement: '', case_sensitive: false }])
    }

    return (
        <div className="space-y-2">
            {value.length > 0 && (
                <div className="grid grid-cols-[1fr_1fr_auto_auto] items-center gap-2">
                    <Label className="text-muted-foreground text-xs">
                        {t('settings.pattern')}
                    </Label>
                    <Label className="text-muted-foreground text-xs">
                        {t('settings.replacement')}
                    </Label>
                    <Label className="text-muted-foreground text-xs">
                        {t('settings.caseSensitive')}
                    </Label>
                    <span />
                </div>
            )}
            {value.map((rule, i) => (
                <div
                    key={i}
                    className="grid grid-cols-[1fr_1fr_auto_auto] items-center gap-2"
                >
                    <Input
                        value={rule.pattern}
                        onChange={(e) => update(i, { pattern: e.target.value })}
                        placeholder={t('settings.pattern')}
                    />
                    <Input
                        value={rule.replacement}
                        onChange={(e) => update(i, { replacement: e.target.value })}
                        placeholder={t('settings.replacement')}
                    />
                    <Checkbox
                        checked={rule.case_sensitive}
                        onCheckedChange={(checked) =>
                            update(i, { case_sensitive: checked === true })
                        }
                    />
                    <Button
                        variant="ghost"
                        size="icon-sm"
                        onClick={() => remove(i)}
                        type="button"
                    >
                        <Trash2 className="size-4" />
                    </Button>
                </div>
            ))}
            {value.length === 0 && (
                <p className="text-muted-foreground text-sm">
                    {t('settings.noRules')}
                </p>
            )}
            <Button variant="outline" size="sm" onClick={add} type="button">
                <Plus className="size-4" />
                {t('settings.addRule')}
            </Button>
        </div>
    )
}
