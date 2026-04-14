import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Loader2 } from 'lucide-react'
import type { GuildSummaryDto } from '@zako-ac/zako3-data'
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
} from '@/components/ui/dialog'
import {
    Command,
    CommandInput,
    CommandList,
    CommandItem,
    CommandEmpty,
} from '@/components/ui/command'
import { Avatar, AvatarImage, AvatarFallback } from '@/components/ui/avatar'

interface GuildSelectDialogProps {
    open: boolean
    onOpenChange: (open: boolean) => void
    onSelect: (guild: GuildSummaryDto) => void
    guilds: GuildSummaryDto[]
    isLoading?: boolean
}

export const GuildSelectDialog = ({
    open,
    onOpenChange,
    onSelect,
    guilds,
    isLoading = false,
}: GuildSelectDialogProps) => {
    const { t } = useTranslation()
    const [search, setSearch] = useState('')

    const filtered = guilds.filter((guild) =>
        guild.guildName.toLowerCase().includes(search.toLowerCase())
    )

    const handleSelect = (guild: GuildSummaryDto) => {
        onSelect(guild)
        onOpenChange(false)
        setSearch('')
    }

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="max-w-sm p-0">
                <div className="p-6 pb-0">
                    <DialogHeader>
                        <DialogTitle>{t('admin.userSettings.selectGuild')}</DialogTitle>
                        <DialogDescription>
                            {t('admin.userSettings.selectGuildDesc')}
                        </DialogDescription>
                    </DialogHeader>
                </div>

                {isLoading ? (
                    <div className="flex items-center justify-center p-8 text-muted-foreground">
                        <Loader2 className="h-4 w-4 animate-spin mr-2" />
                        {t('common.loading')}
                    </div>
                ) : (
                    <Command shouldFilter={false}>
                        <CommandInput
                            placeholder={t('admin.userSettings.guildSearchPlaceholder')}
                            value={search}
                            onValueChange={setSearch}
                        />
                        <CommandList>
                            {filtered.length === 0 ? (
                                <CommandEmpty>
                                    {t('admin.userSettings.noGuilds')}
                                </CommandEmpty>
                            ) : (
                                filtered.map((guild) => (
                                    <CommandItem
                                        key={guild.guildId}
                                        value={guild.guildId}
                                        onSelect={() => handleSelect(guild)}
                                        className="cursor-pointer"
                                    >
                                        <Avatar className="h-8 w-8 mr-3">
                                            <AvatarImage src={guild.guildIconUrl} alt={guild.guildName} />
                                            <AvatarFallback>
                                                {guild.guildName.slice(0, 2).toUpperCase()}
                                            </AvatarFallback>
                                        </Avatar>
                                        <span>{guild.guildName}</span>
                                    </CommandItem>
                                ))
                            )}
                        </CommandList>
                    </Command>
                )}
            </DialogContent>
        </Dialog>
    )
}
