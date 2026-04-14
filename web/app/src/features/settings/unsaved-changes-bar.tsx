import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { AlertTriangle } from 'lucide-react'

interface UnsavedChangesBarProps {
    onSave: () => void
    onReset: () => void
    isSaving: boolean
}

export function UnsavedChangesBar({ onSave, onReset, isSaving }: UnsavedChangesBarProps) {
    const { t } = useTranslation()

    return (
        <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-50 animate-in slide-in-from-bottom-4 bg-yellow-50 dark:bg-yellow-950/30 border border-yellow-200 dark:border-yellow-800 rounded-lg shadow-lg p-4 flex items-center gap-4 max-w-md">
            <AlertTriangle className="h-5 w-5 text-yellow-600 dark:text-yellow-400 flex-shrink-0" />
            <div className="flex-1">
                <p className="text-sm font-medium text-yellow-900 dark:text-yellow-300">
                    {t('settings.unsavedChanges')}
                </p>
            </div>
            <div className="flex gap-2 flex-shrink-0">
                <Button
                    variant="outline"
                    size="sm"
                    onClick={onReset}
                    disabled={isSaving}
                >
                    {t('settings.reset')}
                </Button>
                <Button
                    size="sm"
                    onClick={onSave}
                    disabled={isSaving}
                >
                    {isSaving ? t('settings.saving') : t('settings.save')}
                </Button>
            </div>
        </div>
    )
}
