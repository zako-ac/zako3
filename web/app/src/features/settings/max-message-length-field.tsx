import { useTranslation } from 'react-i18next'
import { Input } from '@/components/ui/input'

interface MaxMessageLengthFieldProps {
    value: number
    onChange: (value: number) => void
}

export function MaxMessageLengthField({ value, onChange }: MaxMessageLengthFieldProps) {
    const { t } = useTranslation()

    return (
        <div className="space-y-1">
            <Input
                type="number"
                min={1}
                max={65535}
                value={value}
                onChange={(e) => {
                    const n = parseInt(e.target.value, 10)
                    if (!isNaN(n) && n >= 1 && n <= 65535) onChange(n)
                }}
                className="w-32"
            />
            <p className="text-muted-foreground text-xs">
                {t('settings.maxMessageLengthDescription')}
            </p>
        </div>
    )
}
