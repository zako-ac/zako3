import { useTranslation } from 'react-i18next'
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from '@/components/ui/select'
import type { TapWithAccess } from '@zako-ac/zako3-data'

const NONE_VALUE = '__none__'

interface TtsVoiceFieldProps {
    value: string | null
    onChange: (value: string | null) => void
    taps: TapWithAccess[]
    filterOwnerOnly?: boolean
}

export function TtsVoiceField({ value, onChange, taps, filterOwnerOnly }: TtsVoiceFieldProps) {
    const { t } = useTranslation()

    const ttsTaps = taps
        .filter((tap) => tap.roles.includes('tts'))
        .filter((tap) => !filterOwnerOnly || tap.permission.type !== 'owner_only')

    return (
        <Select
            value={value ?? NONE_VALUE}
            onValueChange={(v) => onChange(v === NONE_VALUE ? null : v)}
        >
            <SelectTrigger className="w-full">
                <SelectValue placeholder={t('settings.ttsVoiceNone')} />
            </SelectTrigger>
            <SelectContent>
                <SelectItem value={NONE_VALUE}>{t('settings.ttsVoiceNone')}</SelectItem>
                {ttsTaps.map((tap) => (
                    <SelectItem key={tap.id} value={tap.id}>
                        {tap.name}
                    </SelectItem>
                ))}
            </SelectContent>
        </Select>
    )
}
