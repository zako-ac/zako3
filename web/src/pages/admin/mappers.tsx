import { useState } from 'react'
import { Link } from 'react-router-dom'
import { Plus, GitBranch } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { MapperList, MapperUploadForm } from '@/features/mappers'
import { ROUTES } from '@/lib/constants'

export const AdminMappersPage = () => {
    const { t } = useTranslation()
    const [uploading, setUploading] = useState(false)

    return (
        <div className="space-y-6">
            <div className="flex items-start justify-between">
                <div>
                    <h1 className="text-2xl font-semibold">{t('admin.mappers.title')}</h1>
                    <p className="text-muted-foreground text-sm">
                        {t('admin.mappers.subtitle')}
                    </p>
                </div>
                <div className="flex gap-2">
                    <Button variant="outline" asChild>
                        <Link to={ROUTES.ADMIN_MAPPERS_PIPELINE}>
                            <GitBranch className="mr-2 h-4 w-4" />
                            {t('admin.mappers.pipelineButton')}
                        </Link>
                    </Button>
                    <Button onClick={() => setUploading(true)} disabled={uploading}>
                        <Plus className="mr-2 h-4 w-4" />
                        {t('admin.mappers.uploadButton')}
                    </Button>
                </div>
            </div>

            {uploading && (
                <MapperUploadForm
                    onSuccess={() => setUploading(false)}
                    onCancel={() => setUploading(false)}
                />
            )}

            <MapperList />
        </div>
    )
}
