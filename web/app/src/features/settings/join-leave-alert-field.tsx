import { useTranslation } from 'react-i18next'
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from '@/components/ui/select'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import type { UserJoinLeaveAlert } from './types'

interface JoinLeaveAlertFieldProps {
    value: UserJoinLeaveAlert
    onChange: (value: UserJoinLeaveAlert) => void
}

type AlertType = UserJoinLeaveAlert['type']

const DEFAULT_FOR_TYPE: Record<AlertType, UserJoinLeaveAlert> = {
    auto: { type: 'auto' },
    off: { type: 'off' },
    with_different_username: { type: 'with_different_username', value: '' },
    custom: { type: 'custom', value: { join_message: '', leave_message: '' } },
}

export function JoinLeaveAlertField({ value, onChange }: JoinLeaveAlertFieldProps) {
    const { t } = useTranslation()

    const handleTypeChange = (type: string) => {
        onChange(DEFAULT_FOR_TYPE[type as AlertType])
    }

    return (
        <div className="space-y-3 w-full">
            <Select value={value.type} onValueChange={handleTypeChange}>
                <SelectTrigger className="w-full">
                    <SelectValue />
                </SelectTrigger>
                <SelectContent>
                    <SelectItem value="auto">{t('settings.joinLeaveAlertAuto')}</SelectItem>
                    <SelectItem value="off">{t('settings.joinLeaveAlertOff')}</SelectItem>
                    <SelectItem value="with_different_username">
                        {t('settings.joinLeaveAlertWithUsername')}
                    </SelectItem>
                    <SelectItem value="custom">{t('settings.joinLeaveAlertCustom')}</SelectItem>
                </SelectContent>
            </Select>

            {value.type === 'with_different_username' && (
                <div className="space-y-1">
                    <Label className="text-muted-foreground text-xs">
                        {t('settings.joinLeaveAlertUsername')}
                    </Label>
                    <Input
                        value={value.value}
                        onChange={(e) =>
                            onChange({ type: 'with_different_username', value: e.target.value })
                        }
                        placeholder={t('settings.joinLeaveAlertUsernamePlaceholder')}
                    />
                </div>
            )}

            {value.type === 'custom' && (
                <div className="space-y-2">
                    <div className="space-y-1">
                        <Label className="text-muted-foreground text-xs">
                            {t('settings.joinLeaveAlertJoinMessage')}
                        </Label>
                        <Input
                            value={value.value.join_message}
                            onChange={(e) =>
                                onChange({
                                    type: 'custom',
                                    value: { ...value.value, join_message: e.target.value },
                                })
                            }
                            placeholder={t('settings.joinLeaveAlertJoinMessagePlaceholder')}
                        />
                    </div>
                    <div className="space-y-1">
                        <Label className="text-muted-foreground text-xs">
                            {t('settings.joinLeaveAlertLeaveMessage')}
                        </Label>
                        <Input
                            value={value.value.leave_message}
                            onChange={(e) =>
                                onChange({
                                    type: 'custom',
                                    value: { ...value.value, leave_message: e.target.value },
                                })
                            }
                            placeholder={t('settings.joinLeaveAlertLeaveMessagePlaceholder')}
                        />
                    </div>
                </div>
            )}
        </div>
    )
}
