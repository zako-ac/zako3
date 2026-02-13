import { useTranslation } from 'react-i18next'
import { SearchInput, FilterDropdown, SortDropdown } from '@/components/common'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import type { TapSort, TapRole } from '@zako-ac/zako3-data'

export interface TapFiltersProps {
    search: string
    onSearchChange: (search: string) => void
    roles: TapRole[]
    onRolesChange: (roles: TapRole[]) => void
    accessible: boolean | undefined
    onAccessibleChange: (accessible: boolean | undefined) => void
    sortField: TapSort['field']
    sortDirection: TapSort['direction']
    onSortFieldChange: (field: TapSort['field']) => void
    onSortDirectionChange: (direction: TapSort['direction']) => void
}

export const TapFiltersComponent = ({
    search,
    onSearchChange,
    roles,
    onRolesChange,
    accessible,
    onAccessibleChange,
    sortField,
    sortDirection,
    onSortFieldChange,
    onSortDirectionChange,
}: TapFiltersProps) => {
    const { t } = useTranslation()

    const roleOptions = [
        { value: 'music' as const, label: t('taps.roleLabels.music') },
        { value: 'tts' as const, label: t('taps.roleLabels.tts') },
    ]

    const sortOptions = [
        { value: 'mostUsed' as const, label: t('taps.sort.mostUsed') },
        { value: 'recentlyCreated' as const, label: t('taps.sort.recentlyCreated') },
        { value: 'alphabetical' as const, label: t('taps.sort.alphabetical') },
    ]

    return (
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex flex-1 flex-wrap items-center gap-2">
                <SearchInput
                    value={search}
                    onChange={onSearchChange}
                    className="w-full sm:w-64"
                />
                <FilterDropdown
                    label={t('taps.filters.byRole')}
                    options={roleOptions}
                    selected={roles}
                    onChange={onRolesChange}
                />
                <div className="flex items-center gap-2">
                    <Switch
                        id="accessible"
                        checked={accessible === true}
                        onCheckedChange={(checked) => onAccessibleChange(checked ? true : undefined)}
                    />
                    <Label htmlFor="accessible" className="text-sm">
                        {t('taps.filters.accessible')}
                    </Label>
                </div>
            </div>

            <SortDropdown
                options={sortOptions}
                value={sortField}
                direction={sortDirection}
                onValueChange={onSortFieldChange}
                onDirectionChange={onSortDirectionChange}
            />
        </div>
    )
}
