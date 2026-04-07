import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'

export type FieldScope = 'none' | 'normal' | 'important'

interface FieldScopeSelectorProps {
    value: FieldScope
    onChange: (value: FieldScope) => void
    showImportant?: boolean
}

export function FieldScopeSelector({ value, onChange, showImportant = false }: FieldScopeSelectorProps) {
    const { t } = useTranslation()

    const options: { label: string; value: FieldScope; activeClass: string }[] = [
        {
            label: t('settings.scope.none'),
            value: 'none',
            activeClass: 'bg-primary text-primary-foreground',
        },
        {
            label: t('settings.scope.normal'),
            value: 'normal',
            activeClass: 'bg-primary text-primary-foreground',
        },
        ...(showImportant
            ? [
                  {
                      label: t('settings.scope.important'),
                      value: 'important' as FieldScope,
                      activeClass: 'bg-amber-500 text-white',
                  },
              ]
            : []),
    ]

    return (
        <div className="flex items-center gap-0.5 rounded-md border p-0.5">
            {options.map((opt) => (
                <button
                    key={opt.value}
                    type="button"
                    onClick={() => onChange(opt.value)}
                    className={cn(
                        'rounded-xs px-2 py-0.5 text-xs font-medium transition-colors',
                        value === opt.value
                            ? opt.activeClass
                            : 'text-muted-foreground hover:text-foreground'
                    )}
                >
                    {opt.label}
                </button>
            ))}
        </div>
    )
}
