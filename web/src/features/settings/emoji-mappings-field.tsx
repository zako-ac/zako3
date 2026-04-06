import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Trash2, Plus } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
    AlertDialog,
    AlertDialogContent,
    AlertDialogHeader,
    AlertDialogTitle,
    AlertDialogFooter,
    AlertDialogCancel,
    AlertDialogAction,
} from '@/components/ui/alert-dialog'
import type { EmojiMappingRule } from './types'

interface EmojiMappingsFieldProps {
    value: EmojiMappingRule[]
    onChange: (value: EmojiMappingRule[]) => void
}

const emptyDraft = (): EmojiMappingRule => ({
    emoji_id: '',
    emoji_name: '',
    emoji_image_url: '',
    replacement: '',
})

export function EmojiMappingsField({ value, onChange }: EmojiMappingsFieldProps) {
    const { t } = useTranslation()
    const [open, setOpen] = useState(false)
    const [draft, setDraft] = useState<EmojiMappingRule>(emptyDraft)

    const updateReplacement = (index: number, replacement: string) => {
        onChange(value.map((rule, i) => (i === index ? { ...rule, replacement } : rule)))
    }

    const remove = (index: number) => {
        onChange(value.filter((_, i) => i !== index))
    }

    const confirmAdd = () => {
        onChange([...value, draft])
        setOpen(false)
        setDraft(emptyDraft())
    }

    return (
        <div className="space-y-2">
            {value.map((rule, i) => (
                <div key={i} className="flex items-center gap-2">
                    {rule.emoji_image_url && (
                        <img
                            src={rule.emoji_image_url}
                            alt={rule.emoji_name}
                            className="size-6 shrink-0 rounded"
                        />
                    )}
                    <span className="text-muted-foreground w-32 shrink-0 truncate text-sm">
                        {rule.emoji_name || rule.emoji_id}
                    </span>
                    <Input
                        value={rule.replacement}
                        onChange={(e) => updateReplacement(i, e.target.value)}
                        placeholder={t('settings.replacement')}
                        className="flex-1"
                    />
                    <Button
                        variant="ghost"
                        size="icon-sm"
                        onClick={() => remove(i)}
                        type="button"
                    >
                        <Trash2 className="size-4" />
                    </Button>
                </div>
            ))}
            {value.length === 0 && (
                <p className="text-muted-foreground text-sm">
                    {t('settings.noRules')}
                </p>
            )}
            <Button variant="outline" size="sm" onClick={() => setOpen(true)} type="button">
                <Plus className="size-4" />
                {t('settings.addEmojiRule')}
            </Button>

            <AlertDialog open={open} onOpenChange={setOpen}>
                <AlertDialogContent>
                    <AlertDialogHeader>
                        <AlertDialogTitle>{t('settings.addEmojiRule')}</AlertDialogTitle>
                    </AlertDialogHeader>
                    <div className="space-y-3">
                        <div className="space-y-1">
                            <Label>{t('settings.emojiId')}</Label>
                            <Input
                                value={draft.emoji_id}
                                onChange={(e) =>
                                    setDraft((d) => ({ ...d, emoji_id: e.target.value }))
                                }
                                placeholder="123456789"
                            />
                        </div>
                        <div className="space-y-1">
                            <Label>{t('settings.emojiName')}</Label>
                            <Input
                                value={draft.emoji_name}
                                onChange={(e) =>
                                    setDraft((d) => ({ ...d, emoji_name: e.target.value }))
                                }
                                placeholder="my_emoji"
                            />
                        </div>
                        <div className="space-y-1">
                            <Label>{t('settings.emojiImageUrl')}</Label>
                            <Input
                                value={draft.emoji_image_url}
                                onChange={(e) =>
                                    setDraft((d) => ({
                                        ...d,
                                        emoji_image_url: e.target.value,
                                    }))
                                }
                                placeholder="https://cdn.discordapp.com/emojis/..."
                            />
                        </div>
                        <div className="space-y-1">
                            <Label>{t('settings.replacement')}</Label>
                            <Input
                                value={draft.replacement}
                                onChange={(e) =>
                                    setDraft((d) => ({ ...d, replacement: e.target.value }))
                                }
                                placeholder={t('settings.replacement')}
                            />
                        </div>
                    </div>
                    <AlertDialogFooter>
                        <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
                        <AlertDialogAction onClick={confirmAdd} disabled={!draft.emoji_id}>
                            {t('common.confirm')}
                        </AlertDialogAction>
                    </AlertDialogFooter>
                </AlertDialogContent>
            </AlertDialog>
        </div>
    )
}
