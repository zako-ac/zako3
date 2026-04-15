import { useTranslation } from 'react-i18next'
import { Trash2, Plus, Download, Upload } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import { Label } from '@/components/ui/label'
import type { TextMappingRule } from './types'
import { JSONLoadDialog } from './json-load-dialog'

interface TextMappingsFieldProps {
    value: TextMappingRule[]
    onChange: (value: TextMappingRule[]) => void
}

export function TextMappingsField({ value, onChange }: TextMappingsFieldProps) {
    const { t } = useTranslation()
    const [selected, setSelected] = useState<Set<number>>(new Set())
    const [importOpen, setImportOpen] = useState(false)

    const allSelected = value.length > 0 && selected.size === value.length

    const update = (index: number, patch: Partial<TextMappingRule>) => {
        onChange(value.map((rule, i) => (i === index ? { ...rule, ...patch } : rule)))
    }

    const remove = (index: number) => {
        onChange(value.filter((_, i) => i !== index))
        setSelected(prev => {
            const next = new Set<number>()
            for (const s of prev) {
                if (s < index) next.add(s)
                else if (s > index) next.add(s - 1)
            }
            return next
        })
    }

    const add = () => {
        onChange([...value, { pattern: '', replacement: '', case_sensitive: false }])
    }

    const toggleSelect = (i: number) => {
        setSelected(prev => {
            const next = new Set(prev)
            next.has(i) ? next.delete(i) : next.add(i)
            return next
        })
    }

    const toggleAll = () => {
        setSelected(allSelected ? new Set() : new Set(value.map((_, i) => i)))
    }

    const exportSelected = () => {
        const data = [...selected].sort((a, b) => a - b).map(i => {
            const r = value[i]
            const obj: { key: string; value: string; caseSensitive?: boolean } = {
                key: r.pattern,
                value: r.replacement,
            }
            if (r.case_sensitive) obj.caseSensitive = true
            return obj
        })
        const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
        const url = URL.createObjectURL(blob)
        const a = document.createElement('a')
        a.href = url
        a.download = 'text-mappings.json'
        a.click()
        URL.revokeObjectURL(url)
    }

    const handleImportJSON = (json: string) => {
        try {
            const parsed = JSON.parse(json)
            if (!Array.isArray(parsed)) return
            const imported: TextMappingRule[] = parsed.map((item: any) => ({
                pattern: String(item.key ?? ''),
                replacement: String(item.value ?? ''),
                case_sensitive: Boolean(item.caseSensitive ?? false),
            }))
            onChange([...value, ...imported])
        } catch {}
    }

    return (
        <div className="space-y-2">
            {value.length > 0 && (
                <div className="grid grid-cols-[auto_1fr_1fr_auto_auto] items-center gap-2">
                    <Checkbox
                        checked={selected.size > 0 && !allSelected ? 'indeterminate' : allSelected}
                        onCheckedChange={toggleAll}
                    />
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
                    className="grid grid-cols-[auto_1fr_1fr_auto_auto] items-center gap-2"
                >
                    <Checkbox
                        checked={selected.has(i)}
                        onCheckedChange={() => toggleSelect(i)}
                    />
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
            <div className="flex flex-wrap gap-2">
                <Button variant="outline" size="sm" onClick={add} type="button">
                    <Plus className="size-4" />
                    {t('settings.addRule')}
                </Button>
                {selected.size > 0 && (
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={exportSelected}
                        type="button"
                    >
                        <Download className="size-4" />
                        {t('settings.exportSelected')} ({selected.size})
                    </Button>
                )}
                <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setImportOpen(true)}
                    type="button"
                >
                    <Upload className="size-4" />
                    {t('settings.importFromJSON')}
                </Button>
            </div>
            <JSONLoadDialog
                open={importOpen}
                onOpenChange={setImportOpen}
                onLoad={handleImportJSON}
            />
        </div>
    )
}
