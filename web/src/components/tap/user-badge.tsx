import { useTranslation } from 'react-i18next'
import { Copy, Check } from 'lucide-react'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import {
    Tooltip,
    TooltipContent,
    TooltipProvider,
    TooltipTrigger,
} from '@/components/ui/tooltip'
import { useClipboard } from '@/hooks'
import { cn } from '@/lib/utils'

interface UserBadgeProps {
    user: {
        id: string
        username: string
        avatar?: string
    }
    showId?: boolean
    className?: string
    onClick?: (e: React.MouseEvent) => void
}

export const UserBadge = ({
    user,
    showId = false,
    className,
    onClick,
}: UserBadgeProps) => {
    const { t } = useTranslation()
    const { copied, copy } = useClipboard()

    const handleCopy = (e: React.MouseEvent) => {
        if (onClick) {
            onClick(e)
        } else {
            e.stopPropagation()
            copy(user.id)
        }
    }

    return (
        <TooltipProvider>
            <Tooltip>
                <TooltipTrigger asChild>
                    <button
                        className={cn(
                            'text-muted-foreground hover:text-foreground group flex items-center gap-2 text-sm transition-colors',
                            className
                        )}
                        onClick={handleCopy}
                    >
                        <Avatar className="h-5 w-5">
                            <AvatarImage src={user.avatar} alt={user.username} />
                            <AvatarFallback className="text-[10px]">
                                {user.username.slice(0, 2).toUpperCase()}
                            </AvatarFallback>
                        </Avatar>
                        <div className="flex flex-col items-start leading-none">
                            <span className="max-w-25 truncate font-medium">
                                {user.username}
                            </span>
                            {showId && (
                                <span className="text-muted-foreground mt-0.5 font-mono text-[10px]">
                                    {user.id}
                                </span>
                            )}
                        </div>
                        {copied ? (
                            <Check className="text-success h-3 w-3" />
                        ) : (
                            <Copy className="h-3 w-3 opacity-0 transition-opacity group-hover:opacity-100" />
                        )}
                    </button>
                </TooltipTrigger>
                <TooltipContent>
                    {copied ? t('common.copied') : t('common.copyIdToClipboard')}
                </TooltipContent>
            </Tooltip>
        </TooltipProvider>
    )
}
