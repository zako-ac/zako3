import { Badge } from '@/components/ui/badge'
import { Lock, Globe, Users, Ban } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { TapPermissionConfig } from '@zako-ac/zako3-data'

interface PermissionBadgeProps {
    permission: TapPermissionConfig
    hasAccess?: boolean
}


export const PermissionBadge = ({
    permission,
    hasAccess,
}: PermissionBadgeProps) => {
    const { t } = useTranslation()

    const config = {
        owner_only: { icon: Lock, variant: 'secondary' as const },
        public: { icon: Globe, variant: 'default' as const },
        whitelisted: { icon: Users, variant: 'outline' as const },
        blacklisted: { icon: Ban, variant: 'destructive' as const },
    }

    const permissionTranslation = {
        owner_only: t("taps.permissions.owner_only"),
        public: t("taps.permissions.public"),
        whitelisted: t("taps.permissions.whitelisted"),
        blacklisted: t("taps.permissions.blacklisted"),
    }[permission.type]

    const { icon: Icon, variant } = config[permission.type]

    return (
        <Badge variant={variant} className="gap-1">
            <Icon className="h-3 w-3" />
            {permissionTranslation}
            {hasAccess !== undefined && (<>
                <span className="ml-2 text-xs">{hasAccess ? t("taps.permissionsCard.accessYes") : t("taps.permissionsCard.accessNo")}</span>
                <span className="ml-1">{hasAccess ? '✓' : '✗'}</span>
            </>)}
        </Badge>
    )
}
