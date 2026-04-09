import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
    DialogDescription,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { TapCard } from '@/components/tap/tap-card'
import { LoadingSkeleton, EmptyState } from '@/components/common'
import { useTaps } from '@/features/taps'
import type { TapRole } from '@zako-ac/zako3-data'
import { Compass } from 'lucide-react'

interface TapSelectDialogProps {
    open: boolean
    onOpenChange: (open: boolean) => void
    onSelect: (tapId: string) => void
    roles?: TapRole[]
}

export function TapSelectDialog({
    open,
    onOpenChange,
    onSelect,
    roles = ['tts'],
}: TapSelectDialogProps) {
    const { t } = useTranslation()
    const [search, setSearch] = useState('')

    const { data, isLoading } = useTaps({
        roles,
        accessible: true,
        perPage: 50,
        ...(search && { search }),
    })

    const allTaps = data?.data ?? []

    // Filter taps by role on the frontend since backend doesn't support role filtering yet
    const taps = allTaps.filter((tap) =>
        roles.every((role) => tap.roles.includes(role))
    )

    const handleSelect = (tapId: string) => {
        onSelect(tapId)
        onOpenChange(false)
        setSearch('')
    }

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="flex w-[95vw] max-h-[100vh] flex-col sm:w-auto sm:max-h-[80vh] sm:max-w-4xl lg:max-w-6xl">
                <DialogHeader>
                    <DialogTitle>{t('settings.selectTap')}</DialogTitle>
                    <DialogDescription>{t('settings.selectTapDescription')}</DialogDescription>
                </DialogHeader>

                <Input
                    placeholder={t('settings.searchTaps')}
                    value={search}
                    onChange={(e) => setSearch(e.target.value)}
                    className="mb-4"
                />

                <div className="flex-1 overflow-y-auto">
                    {isLoading ? (
                        <LoadingSkeleton count={4} variant="card" />
                    ) : taps.length === 0 ? (
                        <EmptyState
                            icon={<Compass className="h-8 w-8" />}
                            title={t('taps.noTaps')}
                            description={t('settings.noTapsDescription')}
                        />
                    ) : (
                        <div className="grid auto-rows-max gap-4 grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 w-full">
                            {taps.map((tap) => (
                                <TapCard
                                    key={tap.id}
                                    tap={tap}
                                    onReport={() => {}}
                                    onClick={handleSelect}
                                />
                            ))}
                        </div>
                    )}
                </div>
            </DialogContent>
        </Dialog>
    )
}
