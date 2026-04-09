import { Link } from 'react-router-dom'
import { ArrowLeft } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { PipelineManager } from '@/features/mappers'
import { ROUTES } from '@/lib/constants'

export const AdminMappersPipelinePage = () => {
    const { t } = useTranslation()
    return (
        <div className="space-y-6">
            <div className="flex items-start justify-between">
                <div>
                    <h1 className="text-2xl font-semibold">{t('admin.mappers.pipeline.title')}</h1>
                    <p className="text-muted-foreground text-sm">
                        {t('admin.mappers.pipeline.subtitle')}
                    </p>
                </div>
                <Button variant="outline" asChild>
                    <Link to={ROUTES.ADMIN_MAPPERS}>
                        <ArrowLeft className="mr-2 h-4 w-4" />
                        {t('admin.mappers.pipeline.backToMappers')}
                    </Link>
                </Button>
            </div>

            <PipelineManager />
        </div>
    )
}
