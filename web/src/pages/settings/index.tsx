import { useTranslation } from 'react-i18next'
import { useAuthStore } from '@/features/auth'
import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
} from '@/components/ui/card'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Separator } from '@/components/ui/separator'
import { ThemeToggle } from '@/components/layout/theme-toggle'
import { LanguageToggle } from '@/components/layout/language-toggle'
import { Label } from '@/components/ui/label'
import { Button } from '@/components/ui/button'
import { AlertTriangle } from 'lucide-react'

export const SettingsPage = () => {
    const { t } = useTranslation()
    const { user } = useAuthStore()

    if (!user) return null

    return (
        <div className="mx-auto max-w-4xl space-y-6">
            <div>
                <h1 className="text-2xl font-semibold">{t('settings.title')}</h1>
                <p className="text-muted-foreground">{t('settings.subtitle')}</p>
            </div>

            <Card>
                <CardHeader>
                    <CardTitle>{t('settings.profile')}</CardTitle>
                    <CardDescription>{t('settings.profileSubtitle')}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    <div className="flex items-center gap-4">
                        <Avatar className="h-16 w-16">
                            <AvatarImage src={user.avatar} alt={user.username} />
                            <AvatarFallback>
                                {user.username.slice(0, 2).toUpperCase()}
                            </AvatarFallback>
                        </Avatar>
                        <div>
                            <p className="font-semibold">{user.username}</p>
                            <p className="text-muted-foreground text-sm">{user.email}</p>
                            <p className="text-muted-foreground font-mono text-xs">
                                {user.id}
                            </p>
                        </div>
                    </div>
                    <Separator />
                    <div className="space-y-2">
                        <Label>{t('common.role')}</Label>
                        <p className="text-sm">{user.isAdmin ? 'Administrator' : 'User'}</p>
                    </div>
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <CardTitle>{t('settings.appearance')}</CardTitle>
                    <CardDescription>{t('settings.appearanceSubtitle')}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    <div className="flex items-center justify-between">
                        <div className="space-y-0.5">
                            <Label>{t('settings.darkMode')}</Label>
                            <p className="text-muted-foreground text-sm">
                                {t('settings.darkModeSubtitle')}
                            </p>
                        </div>
                        <ThemeToggle />
                    </div>
                    <Separator />
                    <div className="flex items-center justify-between">
                        <div className="space-y-0.5">
                            <Label>{t('settings.language')}</Label>
                            <p className="text-muted-foreground text-sm">
                                {t('settings.languageSubtitle')}
                            </p>
                        </div>
                        <LanguageToggle />
                    </div>
                </CardContent>
            </Card>

            <Card>
                <CardHeader>
                    <CardTitle>{t('settings.notifications')}</CardTitle>
                    <CardDescription>
                        {t('settings.notificationsSubtitle')}
                    </CardDescription>
                </CardHeader>
                <CardContent>
                    <p className="text-muted-foreground text-sm">
                        {t('settings.notificationsCheckBackLater')}
                    </p>
                </CardContent>
            </Card>

            <Card className="border-destructive/50">
                <CardHeader>
                    <CardTitle className="text-destructive flex items-center gap-2">
                        <AlertTriangle className="h-5 w-5" />
                        {t('settings.account')}
                    </CardTitle>
                    <CardDescription>{t('settings.accountSubtitle')}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-4">
                    <div>
                        <Label>{t('settings.deleteAccount')}</Label>
                        <p className="text-muted-foreground mt-1 mb-3 text-sm">
                            {t('settings.deleteAccountWarning')}
                        </p>
                        <Button variant="destructive" disabled>
                            {t('settings.deleteAccount')}
                        </Button>
                    </div>
                </CardContent>
            </Card>
        </div>
    )
}
