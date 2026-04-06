import { useTranslation } from 'react-i18next'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import { Label } from '@/components/ui/label'
import type { TextReadingRule } from './types'
import { TFunction } from 'i18next'

interface TextReadingRuleFieldProps {
    value: TextReadingRule
    onChange: (value: TextReadingRule) => void
}

const options: (t: TFunction<'translation', undefined>) => { value: TextReadingRule; labelKey: string }[] = (t) => [
    { value: 'always', labelKey: t('settings.textReadingRuleAlways') },
    { value: 'in_voice_channel', labelKey: t('settings.textReadingRuleInVoiceChannel') },
    { value: 'on_mic_mute', labelKey: t('settings.textReadingRuleOnMicMute') },
]

export function TextReadingRuleField({ value, onChange }: TextReadingRuleFieldProps) {
    const { t } = useTranslation()

    return (
        <RadioGroup
            value={value}
            onValueChange={(v) => onChange(v as TextReadingRule)}
            className="gap-2"
        >
            {options(t).map((opt) => (
                <div key={opt.value} className="flex items-center gap-2">
                    <RadioGroupItem value={opt.value} id={`text-reading-rule-${opt.value}`} />
                    <Label htmlFor={`text-reading-rule-${opt.value}`} className="font-normal">
                        {opt.labelKey}
                    </Label>
                </div>
            ))}
        </RadioGroup>
    )
}
