import { useTranslation } from 'react-i18next'
import { FolderOpen } from 'lucide-react'
import { useState, useRef } from 'react'
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
    DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'

interface JSONLoadDialogProps {
    open: boolean
    onOpenChange: (open: boolean) => void
    onLoad: (json: string) => void
}

export function JSONLoadDialog({ open, onOpenChange, onLoad }: JSONLoadDialogProps) {
    const { t } = useTranslation()
    const [text, setText] = useState('')
    const fileInputRef = useRef<HTMLInputElement>(null)

    const handleOpenChange = (isOpen: boolean) => {
        onOpenChange(isOpen)
        if (!isOpen) {
            setText('')
        }
    }

    const handleLoadFile = () => {
        fileInputRef.current?.click()
    }

    const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0]
        if (!file) return
        const reader = new FileReader()
        reader.onload = (ev) => {
            const content = ev.target?.result as string
            setText(content)
        }
        reader.readAsText(file)
        e.target.value = ''
    }

    const handleConfirm = () => {
        if (text.trim()) {
            onLoad(text)
            handleOpenChange(false)
        }
    }

    return (
        <Dialog open={open} onOpenChange={handleOpenChange}>
            <DialogContent className="flex w-[95vw] flex-col sm:w-auto sm:max-w-2xl">
                <DialogHeader>
                    <DialogTitle>{t('settings.loadFromJSON')}</DialogTitle>
                </DialogHeader>
                <Textarea
                    value={text}
                    onChange={(e) => setText(e.target.value)}
                    placeholder='[{ "key": "...", "value": "..." }]'
                    className="min-h-48 font-mono text-sm"
                />
                <DialogFooter className="flex-row">
                    <Button
                        variant="outline"
                        onClick={handleLoadFile}
                        type="button"
                    >
                        <FolderOpen className="size-4" />
                        {t('settings.loadFromFile')}
                    </Button>
                    <Button
                        onClick={handleConfirm}
                        type="button"
                        disabled={!text.trim()}
                    >
                        {t('settings.load')}
                    </Button>
                </DialogFooter>
                <input
                    ref={fileInputRef}
                    type="file"
                    accept=".json"
                    className="hidden"
                    onChange={handleFileChange}
                />
            </DialogContent>
        </Dialog>
    )
}
