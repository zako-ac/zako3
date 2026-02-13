import { useState, useMemo } from 'react'
import { useDebounce } from './use-debounce'

/**
 * Options for the useTableFilters hook
 */
interface UseTableFiltersOptions<T> {
  /** Initial filter values */
  initialFilters?: Partial<T>
  /** Debounce delay for search input in milliseconds (default: 300) */
  searchDebounce?: number
}

/**
 * Hook for managing table filter state with built-in search debouncing.
 * Simplifies filter management for data tables and lists.
 *
 * @template T - The type of filter object (must include optional search field)
 *
 * @example
 * ```typescript
 * interface UserFilters {
 *   search?: string
 *   role?: string
 *   isActive?: boolean
 * }
 *
 * const { search, setSearch, updateFilter, activeFilters, resetFilters } =
 *   useTableFilters<UserFilters>({ initialFilters: { isActive: true } })
 *
 * // Use in your component:
 * <SearchInput value={search} onChange={setSearch} />
 * <FilterDropdown
 *   value={activeFilters.role}
 *   onChange={(role) => updateFilter('role', role)}
 * />
 * ```
 */
export function useTableFilters<T extends { search?: string }>(
  options: UseTableFiltersOptions<T> = {}
) {
  const { initialFilters = {} as Partial<T>, searchDebounce = 300 } = options

  const [search, setSearch] = useState(initialFilters.search ?? '')
  const [filters, setFilters] = useState<Partial<T>>(initialFilters)

  const debouncedSearch = useDebounce(search, searchDebounce)

  const activeFilters = useMemo(
    () => ({
      ...filters,
      search: debouncedSearch || undefined,
    }),
    [filters, debouncedSearch]
  )

  const updateFilter = (key: keyof T, value: T[keyof T] | undefined) => {
    setFilters((prev) => ({ ...prev, [key]: value }))
  }

  const resetFilters = () => {
    setSearch('')
    setFilters(initialFilters)
  }

  return {
    search,
    setSearch,
    filters,
    setFilters,
    updateFilter,
    activeFilters,
    resetFilters,
    debouncedSearch,
  }
}
