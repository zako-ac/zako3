import { Check, Mic } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import {
    Tooltip,
    TooltipContent,
    TooltipProvider,
    TooltipTrigger,
} from '@/components/ui/tooltip'
import { useUserSettings, useSaveUserSettings } from '@/features/settings'

interface SetAsMyVoiceProps {
    tapId: string
    hasTtsRole: boolean
    hasAccess: boolean
}

export const SetAsMyVoice = ({ tapId, hasTtsRole, hasAccess }: SetAsMyVoiceProps) => {
    const { t } = useTranslation()
    const { data: settings } = useUserSettings()
    const { mutate: saveSettings, isPending } = useSaveUserSettings()

    const canUse = hasTtsRole && hasAccess
    const isActive = settings?.tts_voice === tapId

    const handleClick = (e: React.MouseEvent) => {
        e.stopPropagation()
        if (!canUse || isActive || !settings) return
        saveSettings({ ...settings, tts_voice: tapId })
    }

    return (
        <TooltipProvider>
            <Tooltip>
                <TooltipTrigger asChild>
                    <Button
                        variant={canUse || isActive ? 'default' : 'outline'}
                        size="icon-sm"
                        className="shrink-0"
                        disabled={!canUse || isActive || isPending}
                        onClick={handleClick}
                    >
                        {isActive ? <Check className="h-4 w-4" /> : <Mic className="h-4 w-4" />}
                    </Button>
                </TooltipTrigger>
                <TooltipContent>
                    {canUse ? t('taps.setAsMyVoice') : t('taps.setAsMyVoiceUnavailable')}
                </TooltipContent>
            </Tooltip>
        </TooltipProvider>
    )
}
