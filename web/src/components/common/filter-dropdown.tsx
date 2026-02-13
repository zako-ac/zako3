import { Filter } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
    DropdownMenu,
    DropdownMenuCheckboxItem,
    DropdownMenuContent,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Badge } from '@/components/ui/badge'

export interface FilterOption<T extends string = string> {
    value: T
    label: string
}

interface FilterDropdownProps<T extends string = string> {
    label: string
    options: FilterOption<T>[]
    selected: T[]
    onChange: (selected: T[]) => void
    className?: string
}

export const FilterDropdown = <T extends string = string>({
    label,
    options,
    selected,
    onChange,
    className,
}: FilterDropdownProps<T>) => {


    const handleToggle = (value: T) => {
        if (selected.includes(value)) {
            onChange(selected.filter((v) => v !== value))
        } else {
            onChange([...selected, value])
        }
    }

    const handleClear = () => {
        onChange([])
    }

    return (
        <DropdownMenu>
            <DropdownMenuTrigger asChild>
                <Button variant="outline" size="sm" className={className}>
                    <Filter className="mr-2 h-4 w-4" />
                    {label}
                    {selected.length > 0 && (
                        <Badge variant="secondary" className="ml-2">
                            {selected.length}
                        </Badge>
                    )}
                </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-48">
                <DropdownMenuLabel className="flex items-center justify-between">
                    {label}
                    {selected.length > 0 && (
                        <Button
                            variant="ghost"
                            size="sm"
                            className="h-auto p-0 text-xs"
                            onClick={handleClear}
                        >
                            Clear
                        </Button>
                    )}
                </DropdownMenuLabel>
                <DropdownMenuSeparator />
                {options.map((option) => (
                    <DropdownMenuCheckboxItem
                        key={option.value}
                        checked={selected.includes(option.value)}
                        onCheckedChange={() => handleToggle(option.value)}
                    >
                        {option.label}
                    </DropdownMenuCheckboxItem>
                ))}
            </DropdownMenuContent>
        </DropdownMenu>
    )
}
