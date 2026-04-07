import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
    Card,
    CardContent,
    CardFooter,
    CardHeader,
    CardTitle,
    CardDescription,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { AlertTriangle, Compass } from 'lucide-react'
import type { TapWithAccess } from '@zako-ac/zako3-data'
import { TextMappingsField } from './text-mappings-field'
import { EmojiMappingsField } from './emoji-mappings-field'
import { TextReadingRuleField } from './text-reading-rule-field'
import { JoinLeaveAlertField } from './join-leave-alert-field'
import { MaxMessageLengthField } from './max-message-length-field'
import { TtsQueueField } from './tts-queue-field'
import { TtsVoiceField } from './tts-voice-field'
import { FieldScopeSelector, type FieldScope } from './field-scope-selector'
import type { PartialUserSettings, UserSettingsField } from './types'
import { defaultUserSettings, emptyPartial } from './types'
import { TapSelectDialog } from './tap-select-dialog'

interface UserSettingsCardProps {
    initialValue: PartialUserSettings
    taps: TapWithAccess[]
    onSave: (settings: PartialUserSettings) => Promise<void>
    isSaving?: boolean
    showImportant?: boolean
    upstreamSettings?: PartialUserSettings
}

// Extract the value from a field, or return the default if None
function getValue<T>(field: UserSettingsField<T>, def: T): T {
    return field.type === 'none' ? def : field.value
}

export function UserSettingsCard({
    initialValue,
    taps,
    onSave,
    isSaving = false,
    showImportant = false,
    upstreamSettings,
}: UserSettingsCardProps) {
    const { t } = useTranslation()
    const [value, setValue] = useState<PartialUserSettings>(initialValue)
    const [dialogOpen, setDialogOpen] = useState(false)

    // Change a field's scope type (None/Normal/Important)
    // When enabling from None, initializes with the default value
    const patchType = (key: keyof PartialUserSettings, newType: FieldScope) => {
        setValue((prev) => {
            const current = prev[key]
            if (newType === 'none') {
                return { ...prev, [key]: { type: 'none' } }
            }
            if (current.type !== 'none') {
                return { ...prev, [key]: { type: newType, value: current.value } }
            }
            // Was None — initialize with default value
            const defaults: Record<keyof PartialUserSettings, unknown> = {
                text_mappings: defaultUserSettings.text_mappings,
                emoji_mappings: defaultUserSettings.emoji_mappings,
                text_reading_rule: defaultUserSettings.text_reading_rule,
                user_join_leave_alert: defaultUserSettings.user_join_leave_alert,
                max_message_length: defaultUserSettings.max_message_length,
                enable_tts_queue: defaultUserSettings.enable_tts_queue,
                tts_voice: defaultUserSettings.tts_voice,
            }
            return { ...prev, [key]: { type: newType, value: defaults[key] } }
        })
    }

    // Update the inner value of a field, preserving its type
    const patchValue = <K extends keyof PartialUserSettings>(
        key: K,
        newValue: Extract<PartialUserSettings[K], { type: 'normal' | 'important' }>['value'],
    ) => {
        setValue((prev) => {
            const current = prev[key]
            if (current.type === 'none') return prev
            return { ...prev, [key]: { type: current.type, value: newValue } as PartialUserSettings[K] }
        })
    }

    const upstream = upstreamSettings ?? emptyPartial

    // Renders the override warning when an upstream scope has Important
    const OverrideAlert = ({ fieldKey }: { fieldKey: keyof PartialUserSettings }) =>
        upstream[fieldKey].type === 'important' ? (
            <Alert className="border-yellow-200 bg-yellow-50 text-yellow-800 dark:border-yellow-800 dark:bg-yellow-950/30 dark:text-yellow-300">
                <AlertTriangle className="!text-yellow-600 dark:!text-yellow-400" />
                <AlertDescription className="text-yellow-700 dark:text-yellow-400">
                    {t('settings.overriddenByUpstream')}
                </AlertDescription>
            </Alert>
        ) : null

    // Wrapper that dims + blocks pointer events when field scope is None
    const FieldWrapper = ({
        fieldKey,
        children,
    }: {
        fieldKey: keyof PartialUserSettings
        children: React.ReactNode
    }) => (
        <div className={value[fieldKey].type === 'none' ? 'pointer-events-none opacity-40' : ''}>
            {children}
        </div>
    )

    return (
        <Card>
            <CardHeader>
                <CardTitle>{t('settings.tts')}</CardTitle>
                <CardDescription>{t('settings.ttsSubtitle')}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-6">
                {/* TTS Voice */}
                <div className="space-y-2">
                    <div className="flex items-center justify-between">
                        <Label>{t('settings.ttsVoice')}</Label>
                        <FieldScopeSelector
                            value={value.tts_voice.type}
                            onChange={(t) => patchType('tts_voice', t)}
                            showImportant={showImportant}
                        />
                    </div>
                    <OverrideAlert fieldKey="tts_voice" />
                    <FieldWrapper fieldKey="tts_voice">
                        <div className="flex w-full items-center justify-between space-x-2">
                            <TtsVoiceField
                                value={getValue(value.tts_voice, defaultUserSettings.tts_voice)}
                                onChange={(v) => patchValue('tts_voice', v)}
                                taps={taps}
                            />
                            <Button variant="outline" onClick={() => setDialogOpen(true)}>
                                <Compass />
                            </Button>
                        </div>
                    </FieldWrapper>
                    <TapSelectDialog
                        open={dialogOpen}
                        onOpenChange={setDialogOpen}
                        onSelect={(tapId) => patchValue('tts_voice', tapId)}
                    />
                </div>

                <Separator />

                {/* TTS Queue */}
                <div className="space-y-2">
                    <div className="flex items-center justify-between">
                        <Label>{t('settings.ttsQueue')}</Label>
                        <FieldScopeSelector
                            value={value.enable_tts_queue.type}
                            onChange={(t) => patchType('enable_tts_queue', t)}
                            showImportant={showImportant}
                        />
                    </div>
                    <OverrideAlert fieldKey="enable_tts_queue" />
                    <FieldWrapper fieldKey="enable_tts_queue">
                        <TtsQueueField
                            value={getValue(value.enable_tts_queue, defaultUserSettings.enable_tts_queue)}
                            onChange={(v) => patchValue('enable_tts_queue', v)}
                        />
                    </FieldWrapper>
                </div>

                <Separator />

                {/* Text Reading Rule */}
                <div className="space-y-2">
                    <div className="flex items-center justify-between">
                        <Label>{t('settings.textReadingRule')}</Label>
                        <FieldScopeSelector
                            value={value.text_reading_rule.type}
                            onChange={(t) => patchType('text_reading_rule', t)}
                            showImportant={showImportant}
                        />
                    </div>
                    <OverrideAlert fieldKey="text_reading_rule" />
                    <FieldWrapper fieldKey="text_reading_rule">
                        <TextReadingRuleField
                            value={getValue(value.text_reading_rule, defaultUserSettings.text_reading_rule)}
                            onChange={(v) => patchValue('text_reading_rule', v)}
                        />
                    </FieldWrapper>
                </div>

                <Separator />

                {/* Join/Leave Alert */}
                <div className="space-y-2">
                    <div className="flex items-center justify-between">
                        <Label>{t('settings.joinLeaveAlert')}</Label>
                        <FieldScopeSelector
                            value={value.user_join_leave_alert.type}
                            onChange={(t) => patchType('user_join_leave_alert', t)}
                            showImportant={showImportant}
                        />
                    </div>
                    <OverrideAlert fieldKey="user_join_leave_alert" />
                    <FieldWrapper fieldKey="user_join_leave_alert">
                        <JoinLeaveAlertField
                            value={getValue(value.user_join_leave_alert, defaultUserSettings.user_join_leave_alert)}
                            onChange={(v) => patchValue('user_join_leave_alert', v)}
                        />
                        <p className="text-muted-foreground text-sm">
                            {t('settings.joinLeaveAlertTapsDescription')}
                        </p>
                    </FieldWrapper>
                </div>

                <Separator />

                {/* Max Message Length */}
                <div className="space-y-2">
                    <div className="flex items-center justify-between">
                        <Label>{t('settings.maxMessageLength')}</Label>
                        <FieldScopeSelector
                            value={value.max_message_length.type}
                            onChange={(t) => patchType('max_message_length', t)}
                            showImportant={showImportant}
                        />
                    </div>
                    <OverrideAlert fieldKey="max_message_length" />
                    <FieldWrapper fieldKey="max_message_length">
                        <MaxMessageLengthField
                            value={getValue(value.max_message_length, defaultUserSettings.max_message_length)}
                            onChange={(v) => patchValue('max_message_length', v)}
                        />
                    </FieldWrapper>
                </div>

                <Separator />

                {/* Text Mappings */}
                <div className="space-y-2">
                    <div className="flex items-center justify-between">
                        <Label>{t('settings.textMappings')}</Label>
                        <FieldScopeSelector
                            value={value.text_mappings.type}
                            onChange={(t) => patchType('text_mappings', t)}
                            showImportant={showImportant}
                        />
                    </div>
                    <p className="text-muted-foreground text-sm">
                        {t('settings.textMappingsDescription')}
                    </p>
                    <OverrideAlert fieldKey="text_mappings" />
                    <FieldWrapper fieldKey="text_mappings">
                        <TextMappingsField
                            value={getValue(value.text_mappings, defaultUserSettings.text_mappings)}
                            onChange={(v) => patchValue('text_mappings', v)}
                        />
                    </FieldWrapper>
                </div>

                <Separator />

                {/* Emoji Mappings */}
                <div className="space-y-2">
                    <div className="flex items-center justify-between">
                        <Label>{t('settings.emojiMappings')}</Label>
                        <FieldScopeSelector
                            value={value.emoji_mappings.type}
                            onChange={(t) => patchType('emoji_mappings', t)}
                            showImportant={showImportant}
                        />
                    </div>
                    <p className="text-muted-foreground text-sm">
                        {t('settings.emojiMappingsDescription')}
                    </p>
                    <OverrideAlert fieldKey="emoji_mappings" />
                    <FieldWrapper fieldKey="emoji_mappings">
                        <EmojiMappingsField
                            value={getValue(value.emoji_mappings, defaultUserSettings.emoji_mappings)}
                            onChange={(v) => patchValue('emoji_mappings', v)}
                        />
                    </FieldWrapper>
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
