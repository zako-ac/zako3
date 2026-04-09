import type { ReactNode } from 'react'
import { SearchInput } from '@/components/common/search-input'

interface DataTableHeaderProps {
  /** Title for the table section */
  title?: string
  /** Description text */
  description?: string
  /** Search value */
  search?: string
  /** Search change handler */
  onSearchChange?: (value: string) => void
  /** Search placeholder */
  searchPlaceholder?: string
  /** Additional filter components */
  filters?: ReactNode
  /** Action buttons (e.g., Create, Export) */
  actions?: ReactNode
}

/**
 * Header component for data tables with optional search and filters.
 * Provides a consistent layout for table controls.
 */
export function DataTableHeader({
  title,
  description,
  search,
  onSearchChange,
  searchPlaceholder = 'Search...',
  filters,
  actions,
}: DataTableHeaderProps) {
  return (
    <div className="space-y-4">
      {(title || description) && (
        <div>
          {title && (
            <h2 className="text-2xl font-bold tracking-tight">{title}</h2>
          )}
          {description && (
            <p className="text-muted-foreground text-sm">{description}</p>
          )}
        </div>
      )}

      {(onSearchChange || filters || actions) && (
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex flex-1 gap-2">
            {onSearchChange && (
              <SearchInput
                value={search ?? ''}
                onChange={onSearchChange}
                placeholder={searchPlaceholder}
                className="max-w-sm"
              />
            )}
            {filters}
          </div>
          {actions && <div className="flex gap-2">{actions}</div>}
        </div>
      )}
    </div>
  )
}
