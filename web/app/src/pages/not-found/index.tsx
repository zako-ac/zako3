import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { FileQuestion } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { ROUTES } from '@/lib/constants'

export const NotFoundPage = () => {
  const { t } = useTranslation()
  return (
    <div className="flex flex-col items-center justify-center min-h-[60vh] text-center space-y-4">
      <FileQuestion className="h-16 w-16 text-muted-foreground" />
      <h1 className="text-4xl font-bold">404</h1>
      <p className="text-muted-foreground">{t('notFound.message')}</p>
      <Button asChild>
        <Link to={ROUTES.DASHBOARD}>{t('notFound.goHome')}</Link>
      </Button>
    </div>
  )
}
