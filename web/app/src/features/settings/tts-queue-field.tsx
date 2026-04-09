import { useTranslation } from 'react-i18next'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'

interface TtsQueueFieldProps {
    value: boolean
    onChange: (value: boolean) => void
}

export function TtsQueueField({ value, onChange }: TtsQueueFieldProps) {
    const { t } = useTranslation()

    return (
        <div className="flex items-center justify-between">
            <div className="space-y-0.5">
                <Label>{t('settings.enableTtsQueue')}</Label>
                <p className="text-muted-foreground text-sm">
                    {t('settings.enableTtsQueueDescription')}
                </p>
            </div>
            <Switch checked={value} onCheckedChange={onChange} />
        </div>
    )
}
