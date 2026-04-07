import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useMappers, useDeleteMapper } from '../hooks'
import { MapperCard } from './MapperCard'

export const MapperList = () => {
    const { t } = useTranslation()
    const { data: mappers, isLoading } = useMappers()
    const { mutate: deleteMapper, isPending: isDeleting } = useDeleteMapper()

    const handleDelete = (id: string) => {
        if (!confirm(t('admin.mappers.deleteConfirm', { id }))) return
        deleteMapper(id, {
            onSuccess: () => toast.success(t('admin.mappers.deleteSuccess')),
            onError: () => toast.error(t('admin.mappers.deleteError')),
        })
    }

    if (isLoading) {
        return <div className="text-muted-foreground text-sm">{t('admin.mappers.loading')}</div>
    }

    if (!mappers || mappers.length === 0) {
        return (
            <div className="text-muted-foreground rounded-lg border border-dashed py-12 text-center text-sm">
                {t('admin.mappers.noMappers')}
            </div>
        )
    }

    return (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {mappers.map((mapper) => (
                <MapperCard
                    key={mapper.id}
                    mapper={mapper}
                    onDelete={handleDelete}
                    isDeleting={isDeleting}
                />
            ))}
        </div>
    )
}
