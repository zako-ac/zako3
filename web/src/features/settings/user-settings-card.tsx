import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
    Card,
    CardContent,
    CardDescription,
    CardFooter,
    CardHeader,
    CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import type { TapWithAccess } from '@zako-ac/zako3-data'
import { TextMappingsField } from './text-mappings-field'
import { EmojiMappingsField } from './emoji-mappings-field'
import { TextReadingRuleField } from './text-reading-rule-field'
import { JoinLeaveAlertField } from './join-leave-alert-field'
import { MaxMessageLengthField } from './max-message-length-field'
import { TtsQueueField } from './tts-queue-field'
import { TtsVoiceField } from './tts-voice-field'
import type { UserSettings } from './types'
import { Compass } from 'lucide-react'
import { Link } from 'react-router-dom'
import { ROUTES } from '@/lib/constants'

interface UserSettingsCardProps {
    initialValue: UserSettings
    taps: TapWithAccess[]
    onSave: (settings: UserSettings) => Promise<void>
    isSaving?: boolean
}

export function UserSettingsCard({
    initialValue,
    taps,
    onSave,
    isSaving = false,
}: UserSettingsCardProps) {
    const { t } = useTranslation()
    const [value, setValue] = useState<UserSettings>(initialValue)

    const patch = <K extends keyof UserSettings>(key: K, v: UserSettings[K]) =>
        setValue((prev) => ({ ...prev, [key]: v }))

    return (
        <Card>
            <CardHeader>
                <CardTitle>{t('settings.tts')}</CardTitle>
                <CardDescription>{t('settings.ttsSubtitle')}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-6">
                <div className="space-y-2">
                    <Label>{t('settings.ttsVoice')}</Label>
                    <div className="w-full flex items-center justify-between space-x-2">
                        <TtsVoiceField
                            value={value.tts_voice}
                            onChange={(v) => patch('tts_voice', v)}
                            taps={taps}
                        />
                        <Link to={ROUTES.TAPS}>
                            <Button variant="outline">
                                <Compass />
                            </Button>
                        </Link>
                    </div>
                </div>

                <Separator />

                <TtsQueueField
                    value={value.enable_tts_queue}
                    onChange={(v) => patch('enable_tts_queue', v)}
                />

                <Separator />

                <div className="space-y-2">
                    <Label>{t('settings.textReadingRule')}</Label>
                    <TextReadingRuleField
                        value={value.text_reading_rule}
                        onChange={(v) => patch('text_reading_rule', v)}
                    />
                </div>

                <Separator />

                <div className="space-y-2">
                    <Label>{t('settings.joinLeaveAlert')}</Label>
                    <JoinLeaveAlertField
                        value={value.user_join_leave_alert}
                        onChange={(v) => patch('user_join_leave_alert', v)}
                    />
                    <p className="text-muted-foreground text-sm">
                        {t('settings.joinLeaveAlertTapsDescription')}
                    </p>
                </div>

                <Separator />

                <div className="space-y-2">
                    <Label>{t('settings.maxMessageLength')}</Label>
                    <MaxMessageLengthField
                        value={value.max_message_length}
                        onChange={(v) => patch('max_message_length', v)}
                    />
                </div>

                <Separator />

                <div className="space-y-2">
                    <Label>{t('settings.textMappings')}</Label>
                    <p className="text-muted-foreground text-sm">
                        {t('settings.textMappingsDescription')}
                    </p>
                    <TextMappingsField
                        value={value.text_mappings}
                        onChange={(v) => patch('text_mappings', v)}
                    />
                </div>

                <Separator />

                <div className="space-y-2">
                    <Label>{t('settings.emojiMappings')}</Label>
                    <p className="text-muted-foreground text-sm">
                        {t('settings.emojiMappingsDescription')}
                    </p>
                    <EmojiMappingsField
                        value={value.emoji_mappings}
                        onChange={(v) => patch('emoji_mappings', v)}
                    />
                </div>
            </CardContent>
            <CardFooter>
                <Button onClick={() => onSave(value)} disabled={isSaving}>
                    {isSaving ? t('settings.saving') : t('settings.save')}
                </Button>
            </CardFooter>
        </Card>
    )
}
