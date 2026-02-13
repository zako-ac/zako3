import { ArrowUpDown, ArrowUp, ArrowDown } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import type { SortDirection } from '@zako-ac/zako3-data'

export interface SortOption<T extends string = string> {
  value: T
  label: string
}

interface SortDropdownProps<T extends string = string> {
  options: SortOption<T>[]
  value: T
  direction: SortDirection
  onValueChange: (value: T) => void
  onDirectionChange: (direction: SortDirection) => void
  className?: string
}

export const SortDropdown = <T extends string = string>({
  options,
  value,
  direction,
  onValueChange,
  onDirectionChange,
  className,
}: SortDropdownProps<T>) => {
  const currentOption = options.find((o) => o.value === value)
  const DirectionIcon = direction === 'asc' ? ArrowUp : ArrowDown

  const toggleDirection = () => {
    onDirectionChange(direction === 'asc' ? 'desc' : 'asc')
  }

  return (
    <div className={`flex items-center gap-1 ${className}`}>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="outline" size="sm">
            <ArrowUpDown className="mr-2 h-4 w-4" />
            {currentOption?.label || 'Sort'}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start">
          <DropdownMenuRadioGroup
            value={value}
            onValueChange={(v) => onValueChange(v as T)}
          >
            {options.map((option) => (
              <DropdownMenuRadioItem key={option.value} value={option.value}>
                {option.label}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenu>
      <Button variant="outline" size="icon-sm" onClick={toggleDirection}>
        <DirectionIcon className="h-4 w-4" />
      </Button>
    </div>
  )
}
